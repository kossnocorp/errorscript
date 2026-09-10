use crate::prelude::*;

mod walk;

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
        walk::files(self.file_patterns()?).await
    }
}
