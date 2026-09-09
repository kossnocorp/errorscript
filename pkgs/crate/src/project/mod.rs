use crate::prelude::*;

mod state;
pub use state::*;

mod file;

mod parse;

mod check;

#[derive(Debug)]
pub struct EscProject {
    pub config: Option<EscConfig>,
    pub resolver: EscResolver,
    pub state: EscProjectState,
}

impl EscProject {
    pub async fn resolve(path: Option<&PathBuf>) -> Result<Self> {
        let config_path = path.cloned();
        let resolver_path = path.cloned();

        let (config, resolver) = tokio::join!(
            tokio::task::spawn_blocking(move || EscConfig::resolve(config_path.as_ref())),
            tokio::task::spawn_blocking(move || EscResolver::resolve(resolver_path.as_ref())),
        );

        let config = config
            .context("ErrorScript config task failed")?
            .context("Failed to load ErrorScript config")?;
        let resolver = resolver
            .context("Resolver task failed")?
            .context("Failed to initialize resolver")?;

        let state = EscProjectState::Resolved;

        Ok(Self {
            config,
            resolver,
            state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn resolve_configs() {
        let project_dir = tempdir().unwrap();
        std::fs::write(project_dir.path().join("errconfig.toml"), "files = []").unwrap();
        std::fs::write(project_dir.path().join("tsconfig.json"), "{}").unwrap();

        let path = project_dir.path().to_path_buf();
        let project = EscProject::resolve(Some(&path)).await.unwrap();

        assert!(project.config.is_some());
        assert!(project.file_patterns().unwrap().is_empty());
    }

    #[tokio::test]
    async fn resolve_without_tsconfig() {
        let project_dir = tempdir().unwrap();
        let path = project_dir.path().to_path_buf();

        let project = EscProject::resolve(Some(&path)).await.unwrap();

        assert!(project.file_patterns().unwrap().is_empty());
    }

    #[tokio::test]
    async fn errconfig_files_override_tsconfig() {
        let project_dir = tempdir().unwrap();
        let esc_dir = project_dir.path().join("esc");
        let ts_dir = project_dir.path().join("ts");
        std::fs::create_dir_all(esc_dir.join("src")).unwrap();
        std::fs::create_dir_all(ts_dir.join("src")).unwrap();

        let esc_path = esc_dir.join("errconfig.toml");
        let ts_path = ts_dir.join("tsconfig.json");
        std::fs::write(&esc_path, "files = [\"src/**/*.esc.ts\"]").unwrap();
        std::fs::write(
            &ts_path,
            r#"{"files":["src/explicit.ts"],"include":["src/**/*.ts"]}"#,
        )
        .unwrap();

        let esc_file = esc_dir.join("src/example.esc.ts");
        let ts_file = ts_dir.join("src/example.ts");
        let ts_explicit_file = ts_dir.join("src/explicit.ts");
        std::fs::write(&esc_file, "").unwrap();
        std::fs::write(&ts_file, "").unwrap();
        std::fs::write(&ts_explicit_file, "").unwrap();

        let resolver = EscResolver::resolve(Some(&ts_path)).unwrap();
        let project = EscProject {
            config: EscConfig::resolve(Some(&esc_path)).unwrap(),
            resolver,
            state: EscProjectState::Resolved,
        };

        let patterns = project.file_patterns().unwrap();
        assert!(patterns.contains(&esc_dir.join("src/**/*.esc.ts")));
        assert_eq!(patterns.len(), 1);

        assert_eq!(project.files().await.unwrap(), module_paths([esc_file]));
    }

    #[tokio::test]
    async fn uses_tsconfig_when_errconfig_files_are_undefined() {
        let project_dir = tempdir().unwrap();
        let src_dir = project_dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();

        std::fs::write(project_dir.path().join("errconfig.toml"), "").unwrap();
        std::fs::write(
            project_dir.path().join("tsconfig.json"),
            r#"{"files":["src/explicit.ts"],"include":["src/**/*.ts"]}"#,
        )
        .unwrap();

        let included_file = src_dir.join("included.ts");
        let explicit_file = src_dir.join("explicit.ts");
        std::fs::write(&included_file, "included").unwrap();
        std::fs::write(&explicit_file, "explicit").unwrap();

        let path = project_dir.path().to_path_buf();
        let mut project = EscProject::resolve(Some(&path)).await.unwrap();

        assert_eq!(
            project.files().await.unwrap(),
            module_paths([included_file.clone(), explicit_file.clone()])
        );
        project.parse_files().await.unwrap();
        if let EscProjectState::Parsed(state) = &project.state {
            assert_eq!(
                state.parsed_files.keys().cloned().collect::<HashSet<_>>(),
                module_paths([included_file, explicit_file])
            );
        } else {
            panic!("Expected parsed state");
        }
    }

    #[tokio::test]
    async fn loads_transitive_dependencies() {
        let project_dir = tempdir().unwrap();
        let src_dir = project_dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::write(
            project_dir.path().join("errconfig.toml"),
            "files = [\"src/entry.ts\"]",
        )
        .unwrap();

        let entry = src_dir.join("entry.ts");
        let middle = src_dir.join("middle.ts");
        let leaf = src_dir.join("leaf.ts");
        std::fs::write(&entry, "import './middle';").unwrap();
        std::fs::write(&middle, "import './leaf';").unwrap();
        std::fs::write(&leaf, "export const leaf = true;").unwrap();

        let path = project_dir.path().to_path_buf();
        let mut project = EscProject::resolve(Some(&path)).await.unwrap();
        project.parse_files().await.unwrap();

        let EscProjectState::Parsed(state) = &project.state else {
            panic!("Expected parsed state");
        };
        assert_eq!(
            state.parsed_files.keys().cloned().collect::<HashSet<_>>(),
            module_paths([entry, middle, leaf])
        );
    }

    #[tokio::test]
    async fn loads_runtime_and_declaration_dependencies() {
        let project_dir = tempdir().unwrap();
        let root = project_dir.path();
        let package = root.join("node_modules/dual");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(root.join("errconfig.toml"), "files = [\"entry.ts\"]").unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"exports":{"types":"./index.d.ts","import":"./index.js","require":"./index.cjs"}}"#,
        )
        .unwrap();
        let fixtures = [
            (
                "entry.ts",
                "import 'dual'; const cjs = require('dual'); import type { T } from './types.js';",
            ),
            ("types.d.ts", "export interface T {}"),
            ("types.js", "throw new Error('type-only dependency');"),
            ("node_modules/dual/index.d.ts", "export * from './leaf.js';"),
            (
                "node_modules/dual/leaf.d.ts",
                "export declare const leaf: number;",
            ),
            ("node_modules/dual/index.js", "export * from './leaf.js';"),
            ("node_modules/dual/leaf.js", "export const leaf = 1;"),
            (
                "node_modules/dual/index.cjs",
                "module.exports = require('./leaf.cjs');",
            ),
            (
                "node_modules/dual/leaf.d.cts",
                "export declare const leaf: number;",
            ),
            (
                "node_modules/dual/leaf.cjs",
                "module.exports = { leaf: 1 }; require('./index.cjs'); import('./dynamic.mjs');",
            ),
            (
                "node_modules/dual/dynamic.mjs",
                "export const dynamic = true;",
            ),
        ];
        for (file, source) in fixtures {
            std::fs::write(root.join(file), source).unwrap();
        }

        let mut project = EscProject::resolve(Some(&root.to_path_buf()))
            .await
            .unwrap();
        project.parse_files().await.unwrap();
        let EscProjectState::Parsed(state) = &project.state else {
            panic!("Expected parsed state");
        };
        assert_eq!(
            state.parsed_files.keys().cloned().collect::<HashSet<_>>(),
            module_paths(
                fixtures
                    .iter()
                    .filter(|(file, _)| *file != "types.js")
                    .map(|(file, _)| root.join(file))
            )
        );
        for module in state.parsed_files.values() {
            assert!(!module.panicked);
            assert!(module.diagnostics.is_empty(), "{:?}", module.diagnostics);
        }
    }

    fn module_paths(paths: impl IntoIterator<Item = PathBuf>) -> HashSet<EscModulePath> {
        paths
            .into_iter()
            .map(|path| EscModulePath::try_new(path).unwrap())
            .collect()
    }
}
