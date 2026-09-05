use crate::prelude::*;

use std::ops::Deref;

use oxc_resolver::{
    ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};
use oxc_span::VALID_EXTENSIONS;

const CONFIG_FILE: &str = "tsconfig.json";
const CONFIG_EXT: &str = "json";
const OTHER_CONFIG_FILE: &str = crate::config::CONFIG_FILE;

#[derive(Clone, Debug)]
pub struct EscResolver {
    inner: Arc<Resolver>,
    config_path: Option<PathBuf>,
}

impl EscResolver {
    pub fn resolve(path: Option<&PathBuf>) -> Result<Self> {
        let config_path = Self::resolve_path(path)?;
        let alias = |extensions: &[&str]| extensions.iter().map(ToString::to_string).collect();
        let resolver = Arc::new(Resolver::new(ResolveOptions {
            extensions: VALID_EXTENSIONS
                .iter()
                .map(|extension| format!(".{extension}"))
                .collect(),
            main_fields: vec!["module".to_string(), "main".to_string()],
            condition_names: vec!["module".to_string(), "import".to_string()],
            extension_alias: vec![
                (
                    ".js".to_string(),
                    alias(&[".ts", ".tsx", ".d.ts", ".js", ".jsx"]),
                ),
                (
                    ".jsx".to_string(),
                    alias(&[".tsx", ".ts", ".d.ts", ".jsx", ".js"]),
                ),
                (".mjs".to_string(), alias(&[".mts", ".d.mts", ".mjs"])),
                (".cjs".to_string(), alias(&[".cts", ".d.cts", ".cjs"])),
            ],
            tsconfig: config_path.as_ref().map(|config_path| {
                TsconfigDiscovery::Manual(TsconfigOptions {
                    config_file: config_path.clone(),
                    references: TsconfigReferences::Auto,
                })
            }),
            ..ResolveOptions::default()
        }));

        if let Some(config_path) = &config_path {
            resolver
                .resolve_tsconfig(config_path)
                .with_context(|| format!("Failed to parse config at {}", config_path.display()))?;
        }

        Ok(Self {
            inner: resolver,
            config_path,
        })
    }

    pub fn file_patterns(&self) -> Result<HashSet<PathBuf>> {
        let Some(config_path) = &self.config_path else {
            return Ok(HashSet::new());
        };
        let tsconfig = self
            .inner
            .resolve_tsconfig(config_path)
            .with_context(|| format!("Failed to parse config at {}", config_path.display()))?;
        let dir = tsconfig.directory();
        let mut patterns = HashSet::new();

        if let Some(files) = &tsconfig.files {
            patterns.extend(files.iter().map(|pattern| dir.join(pattern)));
        }

        match &tsconfig.include {
            Some(include) => {
                patterns.extend(include.iter().map(|pattern| dir.join(pattern)));
            }
            None if tsconfig.files.is_none() => {
                patterns.insert(dir.join("**/*"));
            }
            None => {}
        }

        Ok(patterns)
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

impl Deref for EscResolver {
    type Target = Resolver;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_path_from_file() {
        let project_dir = tempdir().unwrap();
        let config_path = project_dir.path().join("custom.json");
        std::fs::write(&config_path, "{}").unwrap();

        assert_eq!(
            EscResolver::resolve_path(Some(&config_path)).unwrap(),
            Some(config_path)
        );
    }

    #[test]
    fn resolve_path_from_errconfig_file() {
        let project_dir = tempdir().unwrap();
        let config_path = EscResolver::join_to(project_dir.path());
        let errconfig_path = project_dir.path().join(OTHER_CONFIG_FILE);
        std::fs::write(&config_path, "{}").unwrap();
        std::fs::write(&errconfig_path, "").unwrap();

        assert_eq!(
            EscResolver::resolve_path(Some(&errconfig_path)).unwrap(),
            Some(config_path)
        );
    }

    #[test]
    fn resolve_path_from_dir() {
        let project_dir = tempdir().unwrap();
        let config_path = EscResolver::join_to(project_dir.path());
        std::fs::write(&config_path, "{}").unwrap();

        assert_eq!(
            EscResolver::resolve_path(Some(&project_dir.path().to_path_buf())).unwrap(),
            Some(config_path)
        );
    }

    #[test]
    fn resolve_path_from_nested_dir() {
        let project_dir = tempdir().unwrap();
        let config_path = EscResolver::join_to(project_dir.path());
        let nested_dir = project_dir.path().join("one/two");
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(&config_path, "{}").unwrap();

        assert_eq!(
            EscResolver::resolve_path(Some(&nested_dir)).unwrap(),
            Some(config_path)
        );
    }

    #[test]
    fn resolve_path_missing() {
        let project_dir = tempdir().unwrap();

        assert_eq!(
            EscResolver::resolve_path(Some(&project_dir.path().to_path_buf())).unwrap(),
            None
        );
    }

    #[test]
    fn resolve_with_tsconfig() {
        let project_dir = tempdir().unwrap();
        let config_path = EscResolver::join_to(project_dir.path());
        std::fs::write(&config_path, "{}").unwrap();

        let resolver = EscResolver::resolve(Some(&config_path)).unwrap();

        assert_eq!(
            resolver.file_patterns().unwrap(),
            HashSet::from([project_dir.path().join("**/*")])
        );
    }

    #[test]
    fn resolve_without_tsconfig() {
        let project_dir = tempdir().unwrap();

        let resolver = EscResolver::resolve(Some(&project_dir.path().to_path_buf())).unwrap();

        assert!(resolver.file_patterns().unwrap().is_empty());
    }

    #[test]
    fn resolve_parse_error() {
        let project_dir = tempdir().unwrap();
        let config_path = EscResolver::join_to(project_dir.path());
        std::fs::write(&config_path, "{").unwrap();

        let error = EscResolver::resolve(Some(&config_path)).unwrap_err();

        assert!(error.to_string().contains("Failed to parse config at"));
    }
}
