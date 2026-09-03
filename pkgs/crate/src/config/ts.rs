use crate::prelude::*;

use oxc_resolver::TsConfig;
use oxc_type_checker::tsoptions::parse_config_file;

pub const CONFIG_FILE: &str = "tsconfig.json";
const CONFIG_EXT: &str = "json";
const OTHER_CONFIG_FILE: &str = super::CONFIG_FILE;

#[derive(Debug)]
pub struct EscConfigTs(pub Arc<TsConfig>);

impl EscConfigTs {
    pub fn resolve(path: Option<&PathBuf>) -> Result<Option<Self>> {
        let config_path = Self::resolve_path(path)?;

        config_path
            .map(|config_path| {
                parse_config_file(&config_path)
                    .map(Self)
                    .with_context(|| format!("Failed to parse config at {}", config_path.display()))
            })
            .transpose()
    }

    fn join_to(path: &Path) -> PathBuf {
        path.join(CONFIG_FILE)
    }

    fn resolve_path(path: Option<&PathBuf>) -> Result<Option<PathBuf>> {
        let cwd = std::env::current_dir().context("Failed to resolve the current directory")?;

        let initial_path = match path {
            Some(path) if path.is_absolute() => path.clone(),
            Some(path) => cwd.join(path),
            None => cwd,
        };

        if initial_path.is_file()
            && initial_path
                .extension()
                .is_some_and(|extension| extension == CONFIG_EXT)
            && initial_path
                .file_name()
                .is_some_and(|file_name| file_name != OTHER_CONFIG_FILE)
        {
            return Ok(Some(initial_path));
        }

        Ok(initial_path
            .ancestors()
            .map(Self::join_to)
            .find(|candidate| candidate.is_file()))
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

        let config_path = EscConfigTs::join_to(project_dir.path());

        assert_eq!(config_path, project_dir.path().join(CONFIG_FILE));
    }

    #[test]
    fn resolve_path_from_file() {
        let project_dir = tempdir().unwrap();
        let config_path = project_dir.path().join("custom.json");
        std::fs::write(&config_path, "{}").unwrap();

        assert_eq!(
            EscConfigTs::resolve_path(Some(&config_path)).unwrap(),
            Some(config_path)
        );
    }

    #[test]
    fn resolve_path_from_errconfig_file() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfigTs::join_to(project_dir.path());
        let errconfig_path = project_dir.path().join(OTHER_CONFIG_FILE);
        std::fs::write(&config_path, "{}").unwrap();
        std::fs::write(&errconfig_path, "").unwrap();

        assert_eq!(
            EscConfigTs::resolve_path(Some(&errconfig_path)).unwrap(),
            Some(config_path)
        );
    }

    #[test]
    fn resolve_path_from_dir() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfigTs::join_to(project_dir.path());
        std::fs::write(&config_path, "{}").unwrap();

        assert_eq!(
            EscConfigTs::resolve_path(Some(&project_dir.path().to_path_buf())).unwrap(),
            Some(config_path)
        );
    }

    #[test]
    fn resolve_path_from_nested_dir() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfigTs::join_to(project_dir.path());
        let nested_dir = project_dir.path().join("one/two");
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(&config_path, "{}").unwrap();

        assert_eq!(
            EscConfigTs::resolve_path(Some(&nested_dir)).unwrap(),
            Some(config_path)
        );
    }

    #[test]
    fn resolve_path_from_cwd() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfigTs::join_to(project_dir.path());
        std::fs::write(&config_path, "{}").unwrap();

        let _cwd = Cwd::set(project_dir.path()).unwrap();

        assert_eq!(EscConfigTs::resolve_path(None).unwrap(), Some(config_path));
    }

    #[test]
    fn resolve_path_from_nested_cwd() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfigTs::join_to(project_dir.path());
        let nested_dir = project_dir.path().join("one/two");
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(&config_path, "{}").unwrap();

        let _cwd = Cwd::set(&nested_dir).unwrap();

        assert_eq!(EscConfigTs::resolve_path(None).unwrap(), Some(config_path));
    }

    #[test]
    fn resolve_path_missing() {
        let project_dir = tempdir().unwrap();

        assert_eq!(
            EscConfigTs::resolve_path(Some(&project_dir.path().to_path_buf())).unwrap(),
            None
        );
    }

    #[test]
    fn resolve_parse_config() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfigTs::join_to(project_dir.path());
        std::fs::write(
            &config_path,
            indoc! {r#"
                {
                  "include": ["src/**/*.ts", "src/**/*.js"]
                }
            "#},
        )
        .unwrap();

        let config = EscConfigTs::resolve(Some(&config_path)).unwrap().unwrap();

        assert_eq!(
            config.0.include,
            Some(vec![
                project_dir.path().join("src/**/*.ts"),
                project_dir.path().join("src/**/*.js"),
            ])
        );
    }

    #[test]
    fn resolve_parse_error() {
        let project_dir = tempdir().unwrap();
        let config_path = EscConfigTs::join_to(project_dir.path());
        std::fs::write(&config_path, "{").unwrap();

        let error = EscConfigTs::resolve(Some(&config_path)).unwrap_err();

        assert!(error.to_string().contains("Failed to parse config at"));
    }

    struct Cwd {
        original: PathBuf,
    }

    impl Cwd {
        fn set(path: impl AsRef<Path>) -> std::io::Result<Self> {
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
