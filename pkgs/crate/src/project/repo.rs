use crate::prelude::*;

impl EscProject {
    pub(super) fn resolve_repo_path(
        initial_path: &EscPath,
        fallback_path: Option<&EscPath>,
    ) -> Result<EscRepoPath> {
        let directory = if initial_path.as_path().is_file() {
            initial_path
                .parent()
                .context("Project file has no parent directory")?
        } else {
            initial_path.clone()
        };

        // Worktrees and submodules use a .git file instead of a directory.
        if let Some(repo_path) = directory
            .as_path()
            .ancestors()
            .find(|ancestor| ancestor.join(".git").exists())
        {
            return EscRepoPath::try_new(repo_path.to_path_buf());
        }

        let root_path = fallback_path.unwrap_or(&directory);
        root_path.clone().try_into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn resolves_git_roots_from_nested_directories_and_config_files() {
        for worktree in [false, true] {
            let dir = tempdir().unwrap();
            let root = dir.path().join("repo");
            let package = root.join("packages/app");
            let source = package.join("src");
            std::fs::create_dir_all(&source).unwrap();
            if worktree {
                std::fs::write(root.join(".git"), "gitdir: /unused/worktree").unwrap();
            } else {
                std::fs::create_dir(root.join(".git")).unwrap();
            }
            let config = package.join("errconfig.toml");
            std::fs::write(&config, "files = []").unwrap();

            for initial in [source, config] {
                let project = EscProject::resolve(Some(&initial)).await.unwrap();
                assert_eq!(
                    project.repo_path.as_path(),
                    std::fs::canonicalize(&root).unwrap()
                );
            }

            // A nested repository takes precedence over the outer repository.
            std::fs::create_dir(package.join(".git")).unwrap();
            let project = EscProject::resolve(Some(&package)).await.unwrap();
            assert_eq!(
                project.repo_path.as_path(),
                std::fs::canonicalize(package).unwrap()
            );
        }
    }

    #[tokio::test]
    async fn falls_back_to_config_directory_or_requested_directory() {
        for config in [None, Some("errconfig.toml"), Some("tsconfig.json")] {
            let dir = tempdir().unwrap();
            let nested = dir.path().join("src/nested");
            std::fs::create_dir_all(&nested).unwrap();
            if let Some(config) = config {
                let content = if config.ends_with("json") { "{}" } else { "" };
                std::fs::write(dir.path().join(config), content).unwrap();
            }
            let project = EscProject::resolve(Some(&nested)).await.unwrap();
            // Temporary directories can themselves be inside a repository.
            let outer_repo = dir
                .path()
                .ancestors()
                .find(|ancestor| ancestor.join(".git").exists());
            let expected = outer_repo.unwrap_or_else(|| {
                if config.is_some() {
                    dir.path()
                } else {
                    &nested
                }
            });
            assert_eq!(
                project.repo_path.as_path(),
                std::fs::canonicalize(expected).unwrap()
            );
        }
    }

    #[tokio::test]
    async fn stores_relative_ids_for_internal_and_external_dependencies() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("repo");
        let source = root.join("src");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(root.join("errconfig.toml"), "files = [\"src/entry.ts\"]").unwrap();
        std::fs::write(source.join("entry.ts"), "import '../../shared';").unwrap();
        std::fs::write(dir.path().join("shared.ts"), "export const value = 1;").unwrap();

        let mut project = EscProject::resolve(Some(&source)).await.unwrap();
        project.parse_files().await.unwrap();
        let EscProjectState::Parsed(state) = &project.state else {
            panic!("Expected parsed state");
        };
        assert_eq!(
            state
                .parsed_files
                .keys()
                .map(EscModuleId::as_str)
                .collect::<HashSet<_>>(),
            HashSet::from(["src/entry.ts", "../shared.ts"])
        );
    }
}
