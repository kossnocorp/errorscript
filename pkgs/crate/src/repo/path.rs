use crate::prelude::*;

/// An absolute, canonical repository directory with symlinks resolved.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EscRepoPath(EscPath);

impl EscRepoPath {
    pub fn try_new(path: PathBuf) -> Result<Self> {
        EscPath::try_new(path)?.try_into()
    }

    pub fn as_path(&self) -> &Path {
        self.0.as_path()
    }
}

impl TryFrom<EscPath> for EscRepoPath {
    type Error = Error;

    fn try_from(path: EscPath) -> Result<Self> {
        ensure!(
            path.as_path().is_dir(),
            "Repository path is not a directory: {path}"
        );
        Ok(Self(path))
    }
}

impl AsRef<Path> for EscRepoPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl Display for EscRepoPath {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn canonicalizes_repository_directory() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();

        let path = EscRepoPath::try_new(dir.path().join("nested/../.")).unwrap();

        assert!(path.as_path().is_absolute());
        assert_eq!(path.as_path(), std::fs::canonicalize(dir.path()).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn resolves_symlinks_before_parent_components() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("nested")).unwrap();
        std::os::unix::fs::symlink(real.join("nested"), dir.path().join("link")).unwrap();

        let path = EscRepoPath::try_new(dir.path().join("link/..")).unwrap();

        assert_eq!(path.as_path(), std::fs::canonicalize(real).unwrap());
    }

    #[test]
    fn rejects_missing_paths_and_files() {
        let dir = tempdir().unwrap();
        assert!(EscRepoPath::try_new(dir.path().join("missing")).is_err());

        let file = dir.path().join("file");
        std::fs::write(&file, "").unwrap();
        assert!(EscRepoPath::try_new(file).is_err());
    }
}
