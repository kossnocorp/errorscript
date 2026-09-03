use crate::prelude::*;

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
    }
}
