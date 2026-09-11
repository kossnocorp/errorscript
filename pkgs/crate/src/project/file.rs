use crate::prelude::*;

mod walk;

impl EscProject {
    pub fn file_patterns(&self) -> Result<HashSet<PathBuf>> {
        if let Some(config) = &self.config
            && let Some(files) = &config.manifest.files
        {
            return Ok(files
                .iter()
                .filter(|pattern| !pattern.to_str().is_some_and(|text| text.starts_with('!')))
                .map(|pattern| config.dir().join(pattern))
                .collect());
        }

        self.resolver.file_patterns()
    }

    pub async fn files(&self) -> Result<HashSet<EscModulePath>> {
        Ok(self.files_with_exclusions().await?.0)
    }

    pub(super) async fn files_with_exclusions(
        &self,
    ) -> Result<(HashSet<EscModulePath>, HashSet<EscModulePath>)> {
        let mut exclusions = HashSet::new();
        if let Some(config) = &self.config
            && let Some(patterns) = &config.manifest.files
        {
            for pattern in patterns {
                if let Some(pattern) = pattern.to_str().and_then(|text| text.strip_prefix('!')) {
                    anyhow::ensure!(!pattern.is_empty(), "Invalid file pattern: !");
                    exclusions.insert(config.dir().join(pattern));
                }
            }
        }
        // Use the same traversal for both sets so exclusions share glob and
        // canonical-path semantics, including paths reached through symlinks.
        let (mut files, excluded) =
            tokio::try_join!(walk::files(self.file_patterns()?), walk::files(exclusions),)?;
        files.retain(|path| !excluded.contains(path));
        Ok((files, excluded))
    }
}
