use crate::prelude::*;

/// An absolute filesystem path, canonicalized with symlinks resolved.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EscPath(PathBuf);

impl EscPath {
    pub fn try_new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let canonical = std::fs::canonicalize(path)
            .with_context(|| format!("Failed to canonicalize path {}", path.display()))?;
        Ok(Self(canonical))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// A parent of a canonical path is already canonical.
    pub fn parent(&self) -> Option<Self> {
        self.0.parent().map(|parent| Self(parent.to_path_buf()))
    }
}

impl AsRef<Path> for EscPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl Display for EscPath {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0.display(), formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolves_relative_paths() {
        let path = EscPath::try_new(".").unwrap();
        assert!(path.as_path().is_absolute());
        assert_eq!(path.as_path(), std::fs::canonicalize(".").unwrap());
    }

    #[test]
    fn normalizes_files_and_directories() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        std::fs::write(dir.path().join("file"), "").unwrap();

        let root = EscPath::try_new(dir.path().join("nested/../.")).unwrap();
        let file = EscPath::try_new(dir.path().join("nested/../file")).unwrap();
        assert_eq!(root.as_path(), std::fs::canonicalize(dir.path()).unwrap());
        assert_eq!(file.parent(), Some(root));
        assert!(EscPath::try_new(dir.path().join("missing")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn resolves_symlinks_before_parent_components() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("nested")).unwrap();
        std::os::unix::fs::symlink(real.join("nested"), dir.path().join("link")).unwrap();

        let path = EscPath::try_new(dir.path().join("link/..")).unwrap();
        assert_eq!(path.as_path(), std::fs::canonicalize(real).unwrap());
    }
}
