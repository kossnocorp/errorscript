use crate::prelude::*;

use oxc_resolver::{
    ResolveError, ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};
use oxc_span::VALID_EXTENSIONS;

const CONFIG_FILE: &str = "tsconfig.json";
const CONFIG_EXT: &str = "json";
const OTHER_CONFIG_FILE: &str = crate::config::CONFIG_FILE;

#[derive(Clone, Debug)]
pub struct EscResolver {
    import: Arc<EscResolverMode>,
    require: Arc<EscResolverMode>,
    config_path: Option<PathBuf>,
}

/// Oxc resolves one target at a time. Each mode shares its filesystem cache with
/// the other mode and with its type-facing resolver.
#[derive(Debug)]
struct EscResolverMode {
    module: Resolver,
    types: Resolver,
}

impl EscResolverMode {
    fn new(module: Resolver, mut options: ResolveOptions) -> Self {
        options.condition_names.push("types".into());
        // resolve_dts delegates package imports (#aliases) and self references
        // to the general resolver, so those paths need TS substitution too.
        for (extension, aliases) in &mut options.extension_alias {
            *aliases = match extension.as_str() {
                ".js" => [".ts", ".tsx", ".d.ts", ".js"].as_slice(),
                ".jsx" => [".tsx", ".ts", ".d.ts", ".jsx"].as_slice(),
                ".mjs" => [".mts", ".d.mts", ".mjs"].as_slice(),
                ".cjs" => [".cts", ".d.cts", ".cjs"].as_slice(),
                _ => continue,
            }
            .iter()
            .map(ToString::to_string)
            .collect();
        }
        let types = module.clone_with_options(options);
        Self { module, types }
    }
}

impl EscResolver {
    pub fn resolve(path: Option<&PathBuf>) -> Result<Self> {
        let config_path = Self::resolve_path(path)?;
        let alias = |extensions: &[&str]| extensions.iter().map(ToString::to_string).collect();
        // Like Rolldown, runtime resolution prefers actual JS and falls back to
        // TS sources. Declaration substitution belongs to resolve_dts instead.
        let mut options = ResolveOptions {
            extensions: VALID_EXTENSIONS
                .iter()
                .map(|ext| format!(".{ext}"))
                .collect(),
            main_fields: vec!["module".into(), "main".into()],
            condition_names: vec!["import".into(), "node".into()],
            extension_alias: vec![
                (".js".into(), alias(&[".js", ".ts", ".tsx"])),
                (".jsx".into(), alias(&[".jsx", ".ts", ".tsx"])),
                (".mjs".into(), alias(&[".mjs", ".mts"])),
                (".cjs".into(), alias(&[".cjs", ".cts"])),
            ],
            builtin_modules: true,
            tsconfig: config_path.as_ref().map(|config_path| {
                TsconfigDiscovery::Manual(TsconfigOptions {
                    config_file: config_path.clone(),
                    references: TsconfigReferences::Auto,
                })
            }),
            ..ResolveOptions::default()
        };
        let import = Arc::new(EscResolverMode::new(
            Resolver::new(options.clone()),
            options.clone(),
        ));
        options.condition_names = vec!["require".into(), "node".into()];
        options.main_fields = vec!["main".into()];
        let require = Arc::new(EscResolverMode::new(
            import.module.clone_with_options(options.clone()),
            options,
        ));

        if let Some(config_path) = &config_path {
            import
                .module
                .resolve_tsconfig(config_path)
                .with_context(|| format!("Failed to parse config at {}", config_path.display()))?;
        }
        Ok(Self {
            import,
            require,
            config_path,
        })
    }

    pub fn resolve_reference(
        &self,
        importing_file: &EscModulePath,
        info: EscModuleReferenceInfo,
    ) -> EscModuleReference {
        let resolver = if matches!(
            info.kind,
            EscModuleReferenceKind::Require | EscModuleReferenceKind::ImportEquals
        ) {
            &self.require
        } else {
            &self.import
        };
        // Triple-slash paths are relative to the source file, including names
        // without a ./ prefix, and refer to the literal file rather than a pair.
        if info.kind == EscModuleReferenceKind::Path {
            let path = importing_file
                .as_path()
                .parent()
                .unwrap()
                .join(&info.specifier);
            let module = EscModulePath::try_new(path.clone()).ok().or_else(|| {
                resolver
                    .module
                    .resolve_file(importing_file, &path.to_string_lossy())
                    .ok()
                    .and_then(|resolution| {
                        EscModulePath::try_new(resolution.path().to_path_buf()).ok()
                    })
            });
            return match module {
                Some(module) => EscModuleReference::External {
                    info,
                    module,
                    types: None,
                },
                None => EscModuleReference::Unresolved { info },
            };
        }

        let module = resolver
            .module
            .resolve_file(importing_file, &info.specifier);
        if let Err(ResolveError::Builtin { resolved, .. }) = &module {
            return EscModuleReference::Internal {
                info,
                name: resolved.clone(),
            };
        }
        let to_path = |resolution: oxc_resolver::Resolution| {
            oxc_span::SourceType::from_path(resolution.path()).ok()?;
            EscModulePath::try_new(resolution.path().to_path_buf()).ok()
        };
        let module = module.ok().and_then(to_path);
        let types = resolver
            .types
            .resolve_dts(importing_file, &info.specifier)
            .ok()
            .and_then(to_path);

        match (module, types) {
            (Some(module), types) => {
                let types = types.filter(|path| {
                    path != &module
                        && oxc_span::SourceType::from_path(path)
                            .is_ok_and(|source_type| source_type.is_typescript())
                });
                EscModuleReference::External {
                    info,
                    module,
                    types,
                }
            }
            // @types packages, ambient declarations and type-only dependencies
            // may have no executable target. Their primary file is the declaration.
            (None, Some(module)) => EscModuleReference::External {
                info,
                module,
                types: None,
            },
            (None, None) => EscModuleReference::Unresolved { info },
        }
    }

    pub fn file_patterns(&self) -> Result<HashSet<PathBuf>> {
        let Some(config_path) = &self.config_path else {
            return Ok(HashSet::new());
        };
        let tsconfig = self
            .import
            .module
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
