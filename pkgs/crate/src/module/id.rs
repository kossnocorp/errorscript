use crate::prelude::*;

/// A normalized, UTF-8 module path relative to the project's repository root.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EscModuleId(RelativePathBuf);

impl EscModuleId {
    pub fn from_path(path: &EscModulePath, repo_path: &EscRepoPath) -> Result<Self> {
        let relative = path.as_path().relative_to(repo_path).with_context(|| {
            format!("Failed to make module path {path} relative to {repo_path}")
        })?;
        Ok(Self(relative.normalize()))
    }

    pub fn as_relative_path(&self) -> &RelativePath {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for EscModuleId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn normalizes_native_paths_into_module_ids() {
        let dir = tempdir().unwrap();
        let root = EscRepoPath::try_new(dir.path().to_path_buf()).unwrap();
        std::fs::create_dir(root.as_path().join("nested")).unwrap();
        std::fs::write(root.as_path().join("module.ts"), "").unwrap();
        let path = EscModulePath::try_new(root.as_path().join("nested/.././module.ts")).unwrap();

        let id = EscModuleId::from_path(&path, &root).unwrap();
        assert_eq!(id.as_str(), "module.ts");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_module_ids() {
        use std::os::unix::ffi::OsStringExt;

        let dir = tempdir().unwrap();
        let path = dir
            .path()
            .join(OsString::from_vec(b"invalid-\xff.ts".to_vec()));
        std::fs::write(&path, "").unwrap();
        let path = EscModulePath::try_new(path).unwrap();
        let root = EscRepoPath::try_new(dir.path().to_path_buf()).unwrap();
        assert!(EscModuleId::from_path(&path, &root).is_err());
    }
}
