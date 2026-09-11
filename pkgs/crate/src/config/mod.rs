use crate::prelude::*;

pub const CONFIG_FILE: &str = "errconfig.toml";
const CONFIG_EXT: &str = "toml";
const OTHER_CONFIG_FILE: &str = "tsconfig.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct EscConfigManifest {
    /// Globs relative to this config, replacing tsconfig file selection when set.
    /// Leading `!` patterns exclude matches (including imported dependencies),
    /// regardless of order. Empty or negative-only lists select no entry files.
    pub files: Option<Vec<PathBuf>>,
}

#[derive(Debug)]
pub struct EscConfig {
    pub manifest: EscConfigManifest,
    pub file_name: OsString,
    pub dir: PathBuf,
}

impl EscConfig {
    pub fn resolve(path: Option<&PathBuf>) -> Result<Option<Self>> {
        let config_path = Self::resolve_path(path)?;

        let config = match config_path {
            Some((file_name, dir)) => {
                let config_path = dir.join(&file_name);
                let config_toml = std::fs::read_to_string(&config_path).with_context(|| {
                    format!("Failed to read config at {}", config_path.display())
                })?;
                let manifest = toml::from_str(&config_toml).with_context(|| {
                    format!("Failed to parse config at {}", config_path.display())
                })?;
                Some(Self {
                    manifest,
                    file_name,
                    dir,
                })
            }

            None => None,
        };
        Ok(config)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn join_to(path: &Path) -> PathBuf {
        path.join(CONFIG_FILE)
    }

    fn resolve_path(path: Option<&PathBuf>) -> Result<Option<(OsString, PathBuf)>> {
        let cwd = std::env::current_dir().context("Failed to resolve the current directory")?;

        let initial_path = match path {
            Some(path) if path.is_absolute() => path,
            Some(path) => &cwd.join(path),
            None => &cwd,
        };

        let config_path = if initial_path.is_file()
            && initial_path
                .extension()
                .is_some_and(|extension| extension == CONFIG_EXT)
            && initial_path
                .file_name()
                .is_some_and(|file_name| file_name != OTHER_CONFIG_FILE)
        {
            Some(initial_path.clone())
        } else {
            initial_path
                .ancestors()
                .map(Self::join_to)
                .find(|candidate| candidate.is_file())
        };

        config_path
            .map(|config_path| {
                let file_name = config_path
                    .file_name()
                    .context("Resolved ErrorScript config path has no file name")?
                    .to_owned();
                let dir = config_path
                    .parent()
                    .context("Resolved ErrorScript config path has no parent directory")?
                    .to_path_buf();

                Ok((file_name, dir))
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
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
            config_loc(&config_path)
        );
    }

    #[test]
    fn resolve_path_from_tsconfig_file() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfig::join_to(project_dir.path());
        let tsconfig_path = project_dir.path().join(OTHER_CONFIG_FILE);
        std::fs::write(&config_path, "").unwrap();
        std::fs::write(&tsconfig_path, "{}").unwrap();

        assert_eq!(
            EscConfig::resolve_path(Some(&tsconfig_path)).unwrap(),
            config_loc(&config_path)
        );
    }

    #[test]
    fn resolve_path_from_dir() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfig::join_to(project_dir.path());
        std::fs::write(&config_path, "").unwrap();

        assert_eq!(
            EscConfig::resolve_path(Some(&project_dir.path().to_path_buf())).unwrap(),
            config_loc(&config_path)
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
            config_loc(&config_path)
        );
    }

    #[test]
    fn resolve_path_from_cwd() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfig::join_to(project_dir.path());
        std::fs::write(&config_path, "").unwrap();

        let _cwd = Cwd::set(project_dir.path()).unwrap();

        assert_eq!(
            EscConfig::resolve_path(None).unwrap(),
            config_loc(&config_path)
        );
    }

    #[test]
    fn resolve_path_from_nested_cwd() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfig::join_to(project_dir.path());
        let nested_dir = project_dir.path().join("one/two");
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(&config_path, "").unwrap();

        let _cwd = Cwd::set(&nested_dir).unwrap();

        assert_eq!(
            EscConfig::resolve_path(None).unwrap(),
            config_loc(&config_path)
        );
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

        let config = EscConfig::resolve(Some(&config_path)).unwrap().unwrap();

        assert_eq!(config.file_name, CONFIG_FILE);
        assert_eq!(config.dir, project_dir.path());
        assert_eq!(
            config.manifest.files,
            Some(vec![
                PathBuf::from("src/**/*.ts"),
                PathBuf::from("src/**/*.js")
            ])
        );
    }

    #[test]
    fn resolve_parse_error() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfig::join_to(project_dir.path());
        std::fs::write(&config_path, "invalid = [").unwrap();

        let error = EscConfig::resolve(Some(&config_path)).unwrap_err();

        assert!(error.to_string().contains("Failed to parse config at"));
    }

    fn config_loc(config_path: &Path) -> Option<(OsString, PathBuf)> {
        Some((
            config_path.file_name().unwrap().to_owned(),
            config_path.parent().unwrap().to_path_buf(),
        ))
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
