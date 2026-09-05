use crate::prelude::*;

use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EscModulePath(PathBuf);

impl EscModulePath {
    pub fn try_new(path: PathBuf) -> Result<Self> {
        let normalized = std::fs::canonicalize(&path)
            .with_context(|| format!("Failed to canonicalize module path {}", path.display()))?;
        Ok(Self(normalized))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for EscModulePath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl Display for EscModulePath {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn canonicalizes_module_path() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("module.ts");
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        std::fs::write(&file, "").unwrap();

        let path = EscModulePath::try_new(dir.path().join("nested/../module.ts")).unwrap();

        assert_eq!(path.as_path(), std::fs::canonicalize(file).unwrap());
    }
}
