use crate::prelude::*;

const CONFIG_FILE: &str = "errconfig.toml";

#[derive(Debug, Serialize, Deserialize)]
pub struct EscConfig {
    files: Option<Vec<PathBuf>>,
}

impl EscConfig {
    pub fn resolve(path: Option<&PathBuf>) -> Result<Option<Self>> {
        let config_path = Self::resolve_path(path)?;

        let config = match config_path {
            Some(config_path) => {
                let config_toml = std::fs::read_to_string(&config_path).with_context(|| {
                    format!("Failed to read config at {}", config_path.display())
                })?;
                let config = toml::from_str(&config_toml).with_context(|| {
                    format!("Failed to parse config at {}", config_path.display())
                })?;
                Some(config)
            }

            None => None,
        };
        Ok(config)
    }

    fn join_to(path: &Path) -> PathBuf {
        path.join(CONFIG_FILE)
    }

    fn resolve_path(path: Option<&PathBuf>) -> Result<Option<PathBuf>> {
        let cwd = std::env::current_dir().context("Failed to resolve the current directory")?;

        let initial_path = match path {
            Some(path) if path.is_absolute() => path,
            Some(path) => &cwd.join(path),
            None => &cwd,
        };

        let config_path = if initial_path.is_file() {
            Some(initial_path.clone())
        } else {
            initial_path
                .ancestors()
                .map(Self::join_to)
                .find(|candidate| candidate.is_file())
        };

        Ok(config_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use insta::*;
    use tempfile::tempdir;

    #[test]
    fn join_to() {
        let project_dir = tempdir().unwrap();

        let config_path = EscConfig::join_to(project_dir.path());

        assert_eq!(config_path, project_dir.path().join(CONFIG_FILE));
    }

    #[test]
    fn resolve_path_from_file() {
        let project_dir = tempdir().unwrap();
        let config_path = project_dir.path().join("custom.toml");
        std::fs::write(&config_path, "").unwrap();

        assert_eq!(
            EscConfig::resolve_path(Some(&config_path)).unwrap(),
            Some(config_path)
        );
    }

    #[test]
    fn resolve_path_from_nested_dir() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfig::join_to(project_dir.path());
        let nested_dir = project_dir.path().join("one/two");
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(&config_path, "").unwrap();

        assert_eq!(
            EscConfig::resolve_path(Some(&nested_dir)).unwrap(),
            Some(config_path)
        );
    }

    #[test]
    fn resolve_path_from_cwd() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfig::join_to(project_dir.path());
        std::fs::write(&config_path, "").unwrap();

        let _cwd = Cwd::set(project_dir.path()).unwrap();

        assert_eq!(EscConfig::resolve_path(None).unwrap(), Some(config_path));
    }

    #[test]
    fn resolve_path_from_nested_cwd() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfig::join_to(project_dir.path());
        let nested_dir = project_dir.path().join("one/two");
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(&config_path, "").unwrap();

        let _cwd = Cwd::set(&nested_dir).unwrap();

        assert_eq!(EscConfig::resolve_path(None).unwrap(), Some(config_path));
    }

    #[test]
    fn resolve_path_missing() {
        let project_dir = tempdir().unwrap();

        assert_eq!(
            EscConfig::resolve_path(Some(&project_dir.path().to_path_buf())).unwrap(),
            None
        );
    }

    #[test]
    fn resolve_parse_config() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfig::join_to(project_dir.path());
        std::fs::write(
            &config_path,
            indoc! {r#"
                files = ["src/**/*.ts", "src/**/*.js"]
            "#},
        )
        .unwrap();

        let config = EscConfig::resolve(Some(&config_path)).unwrap();

        assert_ron_snapshot!(config, @r#"
        Some(EscConfig(
          files: Some([
            "src/**/*.ts",
            "src/**/*.js",
          ]),
        ))
        "#);
    }

    #[test]
    fn resolve_parse_error() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfig::join_to(project_dir.path());
        std::fs::write(&config_path, "invalid = [").unwrap();

        let error = EscConfig::resolve(Some(&config_path)).unwrap_err();

        assert!(error.to_string().contains("Failed to parse config at"));
    }

    struct Cwd {
        original: std::path::PathBuf,
    }

    impl Cwd {
        fn set(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
            let original = std::env::current_dir()?;
            std::env::set_current_dir(path)?;
            Ok(Self { original })
        }
    }

    impl Drop for Cwd {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.original).expect("failed to restore working directory");
        }
    }
}
