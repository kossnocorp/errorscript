use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{Suite, parse_declarations, synchronize};

#[derive(Debug)]
pub struct Options {
    pub ts_libs: PathBuf,
    pub node_types: PathBuf,
    pub node_version: String,
    pub output: PathBuf,
    pub check: bool,
}

impl Default for Options {
    fn default() -> Self {
        let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        Self {
            ts_libs: package.join("../../subs/errs-vscode/extensions/typescript-language-features/dist/development/server/libs"),
            node_types: package.join("node_modules/@types/node"),
            node_version: "24".into(),
            output: package.join("tests"),
            check: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct Report {
    pub files: usize,
    pub changed: Vec<PathBuf>,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub version: u32,
    source: String,
    pub files: BTreeMap<String, Suite>,
}

/// Plan and parse both inventories and all existing tests before writing any changes.
pub fn generate(options: &Options) -> Result<Report> {
    ensure!(
        !options.node_version.is_empty()
            && options.node_version.bytes().all(|c| c.is_ascii_digit()),
        "Node version must be a major number, such as 24"
    );
    let package_path = options.node_types.join("package.json");
    let package: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&package_path).with_context(|| {
            format!(
                "Read {} (install @errorscript/lib dependencies first)",
                package_path.display()
            )
        })?)?;
    let version = package["version"]
        .as_str()
        .context("@types/node package has no version")?;
    ensure!(
        package["name"] == "@types/node"
            && version.split('.').next() == Some(&options.node_version),
        "Expected @types/node {}.x, found {:?} {version}",
        options.node_version,
        package["name"]
    );
    let ts = discover(&options.ts_libs, false)?;
    let node = discover(&options.node_types, true)?;
    ensure!(
        !ts.is_empty(),
        "No lib.*.d.ts files in {}",
        options.ts_libs.display()
    );
    ensure!(
        !node.is_empty(),
        "No declarations in {}",
        options.node_types.display()
    );
    let mut writes = Vec::new();
    let mut report = Report::default();
    plan(
        &options.output.join("ts"),
        "TypeScript lib.*.d.ts",
        &ts,
        true,
        &mut writes,
        &mut report,
    )?;
    plan(
        &options.output.join("node").join(&options.node_version),
        &format!("@types/node@{version}"),
        &node,
        false,
        &mut writes,
        &mut report,
    )?;
    for (path, text) in writes {
        report.changed.push(path.clone());
        if !options.check {
            fs::create_dir_all(path.parent().context("Output has no parent")?)?;
            fs::write(&path, text).with_context(|| format!("Write {}", path.display()))?;
        }
    }
    Ok(report)
}

fn discover(root: &Path, node: bool) -> Result<BTreeMap<String, Suite>> {
    fn walk(
        root: &Path,
        dir: &Path,
        node: bool,
        result: &mut BTreeMap<String, Suite>,
    ) -> Result<()> {
        let mut entries = fs::read_dir(dir)
            .with_context(|| format!("Read {}", dir.display()))?
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type()?.is_dir() {
                // These are alternative TS-compiler compatibility trees, not Node versions.
                let compatibility_tree = name
                    .strip_prefix("ts")
                    .is_some_and(|version| version.starts_with(|c: char| c.is_ascii_digit()));
                if node && !compatibility_tree && name != "node_modules" {
                    walk(root, &path, node, result)?;
                }
                continue;
            }
            if !name.ends_with(".d.ts") || (!node && !name.starts_with("lib.")) {
                continue;
            }
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            let stem = relative.strip_suffix(".d.ts").unwrap();
            let stem = if node {
                stem
            } else {
                stem.strip_prefix("lib.").unwrap_or(stem)
            };
            let source = fs::read_to_string(&path)?;
            let suite = parse_declarations(&source, node)
                .with_context(|| format!("Parse {}", path.display()))?;
            result.insert(format!("{stem}.test.ts"), suite);
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    walk(root, root, node, &mut result)?;
    Ok(result)
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("Read {}", path.display())),
    }
}

fn plan(
    root: &Path,
    source: &str,
    desired: &BTreeMap<String, Suite>,
    library_suites: bool,
    writes: &mut Vec<(PathBuf, String)>,
    report: &mut Report,
) -> Result<()> {
    let manifest_path = root.join(".manifest.json");
    let old_text = read_optional(&manifest_path)?;
    let previous: Manifest = match &old_text {
        Some(text) => {
            let manifest: Manifest =
                serde_json::from_str(text).context("Read inventory manifest")?;
            ensure!(
                manifest.version == 1,
                "Unsupported inventory manifest version {}",
                manifest.version
            );
            manifest
        }
        None => Manifest::default(),
    };
    // Include retired declaration files so their managed APIs are removed, while unrelated tests,
    // imports and helpers in those files remain. Keep the file rather than deleting user code.
    let mut files = desired.clone();
    for file in previous.files.keys() {
        files.entry(file.clone()).or_default();
    }
    for (file, suite) in &files {
        ensure!(
            Path::new(file)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
                && file.ends_with(".test.ts"),
            "Invalid test path in manifest: {file}"
        );
        let path = root.join(file);
        let original = read_optional(&path)?;
        let previous_suite = previous
            .files
            .get(file)
            .unwrap_or(&Suite::default())
            .clone();
        let generated = if library_suites {
            crate::sync::synchronize_library(
                original.as_deref(),
                &previous_suite,
                suite,
                &library_name(file.strip_suffix(".test.ts").unwrap()),
            )
        } else {
            synchronize(original.as_deref(), &previous_suite, suite)
        }
        .with_context(|| format!("Synchronize {}", path.display()))?;
        report.files += 1;
        if original.as_deref() != Some(&generated) {
            writes.push((path, generated));
        }
    }
    let manifest = Manifest {
        version: 1,
        source: source.into(),
        files,
    };
    let text = format!("{}\n", serde_json::to_string_pretty(&manifest)?);
    if old_text.as_deref() != Some(&text) {
        writes.push((manifest_path, text));
    }
    Ok(())
}

pub(crate) fn library_name(stem: &str) -> String {
    stem.split('.')
        .map(|part| match part {
            "dom" => "DOM".into(),
            "esnext" => "ESNext".into(),
            "webworker" => "WebWorker".into(),
            "scripthost" => "ScriptHost".into(),
            "bigint" => "BigInt".into(),
            "arraybuffer" => "ArrayBuffer".into(),
            "sharedmemory" => "SharedMemory".into(),
            "typedarrays" => "TypedArrays".into(),
            "asynciterable" => "AsyncIterable".into(),
            "asyncgenerator" => "AsyncGenerator".into(),
            "importscripts" => "ImportScripts".into(),
            "weakref" => "WeakRef".into(),
            "regexp" => "RegExp".into(),
            "json" => "JSON".into(),
            "modulesource" => "ModuleSource".into(),
            "wellknown" => "WellKnown".into(),
            _ if part.starts_with("es") => part.to_ascii_uppercase(),
            _ => {
                let mut chars = part.chars();
                chars.next().map_or(String::new(), |first| {
                    format!(
                        "{}{rest}",
                        first.to_ascii_uppercase(),
                        rest = chars.as_str()
                    )
                })
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}
