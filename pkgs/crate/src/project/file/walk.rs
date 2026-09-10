//! Parallel glob traversal over canonical directories. A (directory, pattern
//! state) is visited once, even when symlinks form cycles. Keeping pattern state
//! in the key preserves matches through aliases with different remaining globs.
use crate::prelude::*;
use std::{collections::VecDeque, ffi::OsString, path::Component};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct State {
    pattern: usize,
    offset: usize,
}

type Frontier = Vec<(PathBuf, State)>;

struct Pattern {
    parts: Vec<glob::Pattern>,
}

struct Entry {
    name: OsString,
    /// Canonical parent plus a non-symlink entry, or a resolved symlink target.
    path: PathBuf,
    directory: bool,
}

#[derive(Default)]
struct Directory {
    seen: HashSet<State>,
    pending: Vec<State>,
    listing: Option<Arc<Vec<Entry>>>,
    running: bool,
}

fn enqueue(
    path: PathBuf,
    state: State,
    directories: &mut HashMap<PathBuf, Directory>,
    ready: &mut VecDeque<PathBuf>,
) {
    let directory = directories.entry(path.clone()).or_default();
    if directory.seen.insert(state) {
        if directory.pending.is_empty() && !directory.running {
            ready.push_back(path);
        }
        directory.pending.push(state);
    }
}

fn source_file(path: &Path) -> bool {
    oxc_span::VALID_EXTENSIONS
        .iter()
        .any(|extension| path.extension().is_some_and(|actual| actual == *extension))
}

fn listing(path: &Path) -> Result<Vec<Entry>> {
    let entries = std::fs::read_dir(path)
        .with_context(|| format!("Failed to read directory: {}", path.display()))?;
    let mut result = Vec::new();
    for entry in entries {
        let entry =
            entry.with_context(|| format!("Failed to read directory entry: {}", path.display()))?;
        let kind = entry.file_type()?;
        let mut path = entry.path();
        let directory = if kind.is_symlink() {
            // Resolve at the edge, before descending. A link back to an ancestor
            // reaches the same canonical key instead of expanding a longer path.
            let resolved = match EscPath::try_new(&path) {
                Ok(path) => path,
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
                {
                    continue;
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("Failed to resolve symlink: {}", path.display()));
                }
            };
            path = resolved.as_path().to_path_buf();
            let metadata = std::fs::metadata(&path)?;
            if !metadata.is_dir() && !metadata.is_file() {
                continue;
            }
            metadata.is_dir()
        } else {
            if !kind.is_dir() && !kind.is_file() {
                continue;
            }
            kind.is_dir()
        };
        if directory || source_file(Path::new(&entry.file_name())) {
            result.push(Entry {
                name: entry.file_name(),
                path,
                directory,
            });
        }
    }
    Ok(result)
}

struct Output {
    directory: PathBuf,
    listing: Arc<Vec<Entry>>,
    next: Frontier,
    files: HashSet<EscModulePath>,
}

fn scan(
    directory: PathBuf,
    states: Vec<State>,
    cached: Option<Arc<Vec<Entry>>>,
    patterns: Arc<Vec<Pattern>>,
) -> Result<Output> {
    let entries = match cached {
        Some(entries) => entries,
        None => Arc::new(listing(&directory)?),
    };
    let mut next = Vec::new();
    let mut matched = HashSet::new();
    for state in states {
        let parts = &patterns[state.pattern].parts;
        let mut offsets = vec![state.offset];
        // ** can consume zero path components.
        while offsets
            .last()
            .is_some_and(|&offset| offset < parts.len() && parts[offset].as_str() == "**")
        {
            offsets.push(offsets.last().unwrap() + 1);
        }
        for offset in offsets.into_iter().filter(|&offset| offset < parts.len()) {
            let pattern = &parts[offset];
            for entry in entries.iter() {
                let recursive = pattern.as_str() == "**";
                if !recursive && !pattern.matches_path(Path::new(&entry.name)) {
                    continue;
                }
                if entry.directory {
                    let offset = if recursive { offset } else { offset + 1 };
                    if offset < parts.len() {
                        next.push((
                            entry.path.clone(),
                            State {
                                pattern: state.pattern,
                                offset,
                            },
                        ));
                    }
                } else if !recursive && offset + 1 == parts.len() {
                    matched.insert(entry.path.clone());
                }
            }
        }
    }
    let files = matched
        .into_iter()
        .map(EscModulePath::try_new)
        .collect::<Result<_>>()?;
    Ok(Output {
        directory,
        listing: entries,
        next,
        files,
    })
}

fn prepare(paths: HashSet<PathBuf>) -> Result<(Vec<Pattern>, Frontier)> {
    let mut patterns = Vec::new();
    let mut roots = Vec::new();
    for path in paths {
        let text = path
            .to_str()
            .with_context(|| format!("File pattern is not valid UTF-8: {}", path.display()))?;
        glob::Pattern::new(text).with_context(|| format!("Invalid file pattern: {text}"))?;
        // A trailing separator selects directories, never source files.
        if text.ends_with(std::path::MAIN_SEPARATOR) {
            continue;
        }
        let mut root = PathBuf::new();
        let components = path.components().collect::<Vec<_>>();
        let mut index = 0;
        // Start at the fixed directory prefix rather than walking from /.
        while index + 1 < components.len() {
            let component = components[index];
            if component
                .as_os_str()
                .to_string_lossy()
                .contains(['*', '?', '['])
            {
                break;
            }
            root.push(component.as_os_str());
            index += 1;
        }
        if root.as_os_str().is_empty() {
            root.push(".");
        }
        let root = match EscPath::try_new(&root) {
            Ok(root) => root,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        if !root.as_path().is_dir() {
            continue;
        }
        let parts = components[index..]
            .iter()
            .filter(|part| !matches!(part, Component::CurDir))
            .map(|part| glob::Pattern::new(&part.as_os_str().to_string_lossy()).map_err(Into::into))
            .collect::<Result<Vec<_>>>()?;
        if parts.is_empty() {
            continue;
        }
        roots.push((
            root.as_path().to_path_buf(),
            State {
                pattern: patterns.len(),
                offset: 0,
            },
        ));
        patterns.push(Pattern { parts });
    }
    Ok((patterns, roots))
}

pub(super) async fn files(paths: HashSet<PathBuf>) -> Result<HashSet<EscModulePath>> {
    let (patterns, roots) = tokio::task::spawn_blocking(move || prepare(paths))
        .await
        .context("File pattern preparation task failed")??;
    let patterns = Arc::new(patterns);
    let mut directories = HashMap::new();
    let mut ready = VecDeque::new();
    let mut tasks = tokio::task::JoinSet::new();
    let mut files = HashSet::new();
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    for (path, state) in roots {
        enqueue(path, state, &mut directories, &mut ready);
    }
    loop {
        while tasks.len() < workers
            && let Some(path) = ready.pop_front()
        {
            let directory = directories.get_mut(&path).unwrap();
            directory.running = true;
            let states = std::mem::take(&mut directory.pending);
            let listing = directory.listing.clone();
            let patterns = patterns.clone();
            tasks.spawn_blocking(move || scan(path, states, listing, patterns));
        }
        let Some(result) = tasks.join_next().await else {
            break;
        };
        let result = result.context("File discovery worker failed")??;
        let directory = directories.get_mut(&result.directory).unwrap();
        directory.listing = Some(result.listing);
        directory.running = false;
        if !directory.pending.is_empty() {
            ready.push_back(result.directory);
        }
        files.extend(result.files);
        for (path, state) in result.next {
            enqueue(path, state, &mut directories, &mut ready);
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(root: &Path, path: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }

    #[tokio::test]
    async fn matches_glob_semantics_and_deduplicates_overlapping_patterns() {
        let dir = tempdir().unwrap();
        for path in [
            "root.ts",
            "a/a.ts",
            "a/b.tsx",
            "a/nested/c.d.ts",
            "b/nested/d.js",
            ".hidden/e.ts",
            "a/nested/skip.txt",
        ] {
            write(dir.path(), path);
        }
        for patterns in [
            vec!["**/*.ts"],
            vec!["a/*.ts", "**/*.ts"],
            vec!["**/nested/*"],
            vec!["**"],
            vec!["[ab]/**/*.?s"],
            vec!["missing/**/*.ts"],
            vec!["root.ts"],
            vec!["a/**/../root.ts"],
            vec!["a/*.ts/"],
        ] {
            let paths = patterns
                .iter()
                .map(|pattern| dir.path().join(pattern))
                .collect::<HashSet<_>>();
            let expected = paths
                .iter()
                .flat_map(|path| glob::glob(path.to_str().unwrap()).unwrap())
                .map(Result::unwrap)
                .filter(|path| path.is_file() && source_file(path))
                .map(|path| EscModulePath::try_new(path).unwrap())
                .collect::<HashSet<_>>();
            assert_eq!(files(paths).await.unwrap(), expected, "{patterns:?}");
        }
        assert!(
            files(HashSet::from([dir.path().join("[invalid")]))
                .await
                .is_err()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn follows_symlinks_once_per_pattern_state_including_node_modules() {
        use std::os::unix::fs::symlink;
        let dir = tempdir().unwrap();
        for path in [
            "root.ts",
            "package/source.ts",
            "package/node_modules/dependency/index.ts",
            "package/only/a.ts",
        ] {
            write(dir.path(), path);
        }
        symlink(dir.path(), dir.path().join("package/node_modules/back")).unwrap();
        symlink(dir.path().join("package"), dir.path().join("alias")).unwrap();
        symlink(dir.path().join("missing"), dir.path().join("dangling")).unwrap();
        let paths = HashSet::from([dir.path().join("**/*.ts"), dir.path().join("alias/**/a.ts")]);
        let found = files(paths).await.unwrap();
        assert_eq!(found.len(), 4);
        assert!(
            found.contains(
                &EscModulePath::try_new(
                    dir.path().join("package/node_modules/dependency/index.ts")
                )
                .unwrap()
            )
        );
        // A different remaining pattern must still be matched when it reaches
        // a directory already visited through another alias or cycle.
        let found = files(HashSet::from([dir.path().join("**/back/root.ts")]))
            .await
            .unwrap();
        assert_eq!(
            found,
            HashSet::from([EscModulePath::try_new(dir.path().join("root.ts")).unwrap()])
        );
    }
}
