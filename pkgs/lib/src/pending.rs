use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path},
};

use anyhow::{Context, Result, ensure};
use oxc_allocator::Allocator;
use serde::Serialize;

use crate::{
    Options, Suite,
    inventory::{Manifest, library_name},
    parse,
    sync::{Bindings, Existing, suites},
};

#[derive(Debug, Default)]
pub struct PendingOptions {
    pub limit: usize,
    pub kind: Option<String>,
    pub lib: Option<String>,
    /// Exclusive, lexicographic stable ID cursor; it need not still be pending.
    pub after: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PendingCallable {
    /// Test path relative to --out, followed by :: and the JSON describe path.
    pub id: String,
    pub kind: String,
    pub lib: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_version: Option<String>,
    pub test_file: String,
    pub describe_path: Vec<String>,
    /// One-based line of the first pending test, not a permanent identifier.
    pub line: usize,
    /// Relative to pkgs/lib/docs/evidence; Node paths include the runtime major.
    pub evidence_file: String,
}

/// Read manifests for ownership, then parse actual tests for TODOs. Never generate or write.
pub fn next_pending(options: &Options, pending: &PendingOptions) -> Result<Vec<PendingCallable>> {
    ensure!(pending.limit > 0, "--next must be a positive integer");
    ensure!(
        matches!(pending.kind.as_deref(), None | Some("ts" | "node")),
        "--kind must be ts or node"
    );
    ensure!(
        !options.node_version.is_empty()
            && options.node_version.bytes().all(|c| c.is_ascii_digit()),
        "Node version must be a major number, such as 24"
    );
    let root = options
        .output
        .canonicalize()
        .context("Read test output directory")?;
    let mut files = BTreeMap::new();
    for kind in ["node", "ts"] {
        if pending
            .kind
            .as_deref()
            .is_some_and(|selected| selected != kind)
        {
            continue;
        }
        let prefix = if kind == "node" {
            format!("node/{}", options.node_version)
        } else {
            "ts".into()
        };
        let path = root.join(&prefix).join(".manifest.json");
        let manifest: Manifest =
            serde_json::from_str(&fs::read_to_string(&path).with_context(|| {
                format!("Read {} (generate the inventory first)", path.display())
            })?)
            .with_context(|| format!("Parse {}", path.display()))?;
        ensure!(
            manifest.version == 1,
            "Unsupported manifest version in {}",
            path.display()
        );
        for (file, suite) in manifest.files {
            ensure!(
                Path::new(&file)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                    && file.ends_with(".test.ts"),
                "Invalid test path in manifest: {file}"
            );
            let lib = file.strip_suffix(".test.ts").unwrap().to_owned();
            if pending
                .lib
                .as_ref()
                .is_some_and(|selected| selected != &lib)
            {
                continue;
            }
            files.insert(
                format!("{prefix}/{file}"),
                (kind, prefix.clone(), lib, suite),
            );
        }
    }

    let mut result = Vec::new();
    for (file, (kind, prefix, lib, suite)) in files {
        let mut paths = BTreeMap::new();
        let mut path = if kind == "ts" {
            vec![library_name(&lib)]
        } else {
            vec![]
        };
        callable_paths(&suite, &mut path, &mut paths);
        let ids: BTreeMap<_, _> = paths
            .iter()
            .map(|(key, path)| (format!("{file}::{key}"), path))
            .collect();
        if !ids
            .keys()
            .any(|id| pending.after.as_ref().is_none_or(|after| id > after))
        {
            continue;
        }
        let test_file = root.join(&file);
        let source = fs::read_to_string(&test_file)
            .with_context(|| format!("Read {}", test_file.display()))?;
        let allocator = Allocator::default();
        let program = parse(&allocator, &source, false)
            .with_context(|| format!("Parse {}", test_file.display()))?;
        let bindings = Bindings::from_program(&program, &source);
        let wrapped = if kind == "ts" {
            Suite {
                children: BTreeMap::from([(library_name(&lib), suite)]),
                ..Suite::default()
            }
        } else {
            suite
        };
        let existing = suites(&program.body, &wrapped, &wrapped, &bindings)
            .with_context(|| format!("Inspect {}", test_file.display()))?;
        let evidence = evidence_names(paths.values(), kind == "ts");
        // DOM libraries have thousands of callables: index lines once, not per result.
        let newlines: Vec<_> = source
            .match_indices('\n')
            .map(|(offset, _)| offset)
            .collect();
        // IDs are sorted independently of source line numbers, so edits don't disturb pagination.
        for (id, path) in ids {
            if pending.after.as_ref().is_some_and(|after| &id <= after) {
                continue;
            }
            let found = find_suite(&existing, path)
                .with_context(|| format!("Missing managed suite {id}; regenerate the inventory"))?;
            let Some(offset) = found.pending else {
                continue;
            };
            let key = serde_json::to_string(path)?;
            result.push(PendingCallable {
                id,
                kind: kind.into(),
                lib: lib.clone(),
                node_version: (kind == "node").then(|| options.node_version.clone()),
                test_file: test_file.to_string_lossy().into_owned(),
                describe_path: path.clone(),
                line: newlines.partition_point(|&newline| newline < offset as usize) + 1,
                evidence_file: format!("{prefix}/{lib}/{}.md", evidence[&key]),
            });
            if result.len() == pending.limit {
                return Ok(result);
            }
        }
    }
    Ok(result)
}

fn callable_paths(
    suite: &Suite,
    path: &mut Vec<String>,
    result: &mut BTreeMap<String, Vec<String>>,
) {
    if suite.callable {
        result.insert(serde_json::to_string(path).unwrap(), path.clone());
    }
    for (name, child) in &suite.children {
        path.push(name.clone());
        callable_paths(child, path, result);
        path.pop();
    }
}

fn find_suite<'a>(suites: &'a BTreeMap<String, Existing>, path: &[String]) -> Option<&'a Existing> {
    let suite = suites.get(path.first()?)?;
    if path.len() == 1 {
        Some(suite)
    } else {
        find_suite(&suite.children, &path[1..])
    }
}

fn evidence_names<'a>(
    paths: impl Iterator<Item = &'a Vec<String>>,
    ts: bool,
) -> BTreeMap<String, String> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in paths {
        let slug = path[usize::from(ts)..]
            .iter()
            .map(|part| {
                part.to_lowercase()
                    .bytes()
                    .map(|byte| {
                        if byte.is_ascii_alphanumeric() || b"-_.".contains(&byte) {
                            char::from(byte).to_string()
                        } else {
                            format!("%{byte:02X}")
                        }
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("#");
        groups
            .entry(slug)
            .or_default()
            .push(serde_json::to_string(path).unwrap());
    }
    let mut result = BTreeMap::new();
    for (slug, keys) in groups {
        let suffix = keys.len() > 1 || slug.len() > 180 || matches!(slug.as_str(), "." | "..");
        for key in keys {
            // Stable FNV-1a suffix distinguishes case-fold collisions and bounds filename length.
            let hash = key.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
            let name = if suffix {
                format!("{}~{hash:016x}", &slug[..slug.len().min(180)])
            } else {
                slug.clone()
            };
            result.insert(key, name);
        }
    }
    result
}
