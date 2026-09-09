use crate::prelude::*;

impl EscProject {
    pub fn file_patterns(&self) -> Result<HashSet<PathBuf>> {
        if let Some(config) = &self.config
            && let Some(files) = &config.manifest.files
        {
            return Ok(files
                .iter()
                .map(|pattern| config.dir().join(pattern))
                .collect());
        }

        self.resolver.file_patterns()
    }

    pub async fn files(&self) -> Result<HashSet<EscModulePath>> {
        let patterns = self.file_patterns()?;

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
                        && oxc_span::VALID_EXTENSIONS
                            .iter()
                            .any(|ext| path.extension().is_some_and(|e| e == *ext))
                    {
                        files.insert(EscModulePath::try_new(path)?);
                    }
                }
            }

            Ok(files)
        })
        .await
        .context("File resolution task failed")?
    }
}
