use crate::prelude::*;

const PROJECT_EXTS: [&str; 8] = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

#[derive(Debug)]
pub struct EscProject {
    pub config: Option<EscConfig>,
    pub config_ts: Option<EscConfigTs>,
}

impl EscProject {
    pub async fn resolve(path: Option<&PathBuf>) -> Result<Self> {
        let config_path = path.cloned();
        let config_ts_path = path.cloned();

        let (config, config_ts) = tokio::join!(
            tokio::task::spawn_blocking(move || EscConfig::resolve(config_path.as_ref())),
            tokio::task::spawn_blocking(move || EscConfigTs::resolve(config_ts_path.as_ref())),
        );

        let config = config
            .context("ErrorScript config task failed")?
            .context("Failed to load ErrorScript config")?;
        let config_ts = config_ts
            .context("TypeScript config task failed")?
            .context("Failed to load TypeScript config")?;

        Ok(Self { config, config_ts })
    }

    pub fn file_patterns(&self) -> HashSet<PathBuf> {
        if let Some(config) = &self.config
            && let Some(files) = &config.manifest.files
        {
            return files
                .iter()
                .map(|pattern| config.dir().join(pattern))
                .collect();
        }

        let mut patterns = HashSet::new();

        if let Some(config_ts) = &self.config_ts {
            let tsconfig = &config_ts.tsconfig;

            if let Some(files) = &tsconfig.files {
                patterns.extend(files.iter().map(|pattern| config_ts.dir().join(pattern)));
            }

            match &tsconfig.include {
                Some(include) => {
                    patterns.extend(include.iter().map(|pattern| config_ts.dir().join(pattern)));
                }
                None if tsconfig.files.is_none() => {
                    patterns.insert(config_ts.dir().join("**/*"));
                }
                None => {}
            }
        }

        patterns
    }

    pub async fn files(&self) -> Result<HashSet<PathBuf>> {
        let patterns = self.file_patterns();

        tokio::task::spawn_blocking(move || {
            let mut files = HashSet::new();

            for pattern in patterns {
                let pattern = pattern.to_str().with_context(|| {
                    format!("File pattern is not valid UTF-8: {}", pattern.display())
                })?;
                let entries = glob::glob(pattern)
                    .with_context(|| format!("Invalid file pattern: {pattern}"))?;

                for entry in entries {
                    let path = entry
                        .with_context(|| format!("Failed to resolve file pattern: {pattern}"))?;
                    if path.is_file()
                        && PROJECT_EXTS
                            .iter()
                            .any(|ext| path.extension().is_some_and(|e| e == *ext))
                    {
                        files.insert(path);
                    }
                }
            }

            Ok(files)
        })
        .await
        .context("File resolution task failed")?
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
        assert!(project.config_ts.is_some());
        assert!(project.file_patterns().is_empty());
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

        let project = EscProject {
            config: EscConfig::resolve(Some(&esc_path)).unwrap(),
            config_ts: EscConfigTs::resolve(Some(&ts_path)).unwrap(),
        };

        let patterns = project.file_patterns();
        assert!(patterns.contains(&esc_dir.join("src/**/*.esc.ts")));
        assert_eq!(patterns.len(), 1);

        assert_eq!(project.files().await.unwrap(), HashSet::from([esc_file]));
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
        std::fs::write(&included_file, "").unwrap();
        std::fs::write(&explicit_file, "").unwrap();

        let path = project_dir.path().to_path_buf();
        let project = EscProject::resolve(Some(&path)).await.unwrap();

        assert_eq!(
            project.files().await.unwrap(),
            HashSet::from([included_file, explicit_file])
        );
    }
}
