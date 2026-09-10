use crate::prelude::*;
use std::hash::{Hash, Hasher};

/// A normalized, UTF-8 module path relative to the project's repository root.
#[derive(Clone, Eq, Ord, PartialOrd)]
pub struct EscModuleId(Arc<RelativePathBuf>, u64);

impl PartialEq for EscModuleId {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || (self.1 == other.1 && self.as_str() == other.as_str())
    }
}

impl Hash for EscModuleId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Paths are immutable and normalized. Avoid rewalking every path
        // component for each function, symbol, and summary table lookup.
        state.write_u64(self.1);
    }
}

impl std::fmt::Debug for EscModuleId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("EscModuleId").field(&self.0).finish()
    }
}

impl EscModuleId {
    pub fn from_path(path: &EscModulePath, repo_path: &EscRepoPath) -> Result<Self> {
        let relative = path.as_path().relative_to(repo_path).with_context(|| {
            format!("Failed to make module path {path} relative to {repo_path}")
        })?;
        let relative = relative.normalize();
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        relative.as_str().hash(&mut hash);
        Ok(Self(Arc::new(relative), hash.finish()))
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
    fn cached_hash_preserves_path_identity_and_handles_collisions() {
        let dir = tempdir().unwrap();
        let root = EscRepoPath::try_new(dir.path().to_path_buf()).unwrap();
        std::fs::write(dir.path().join("a.ts"), "").unwrap();
        std::fs::write(dir.path().join("b.ts"), "").unwrap();
        let a = EscModulePath::try_new(dir.path().join("a.ts")).unwrap();
        let first = EscModuleId::from_path(&a, &root).unwrap();
        let second = EscModuleId::from_path(&a, &root).unwrap();
        assert!(!Arc::ptr_eq(&first.0, &second.0));
        assert_eq!(first, second);
        let mut other = EscModuleId::from_path(
            &EscModulePath::try_new(dir.path().join("b.ts")).unwrap(),
            &root,
        )
        .unwrap();
        other.1 = first.1;
        let mut ids = HashSet::from([first.clone()]);
        assert!(!ids.insert(second));
        assert!(ids.insert(other));
        assert!(ids.contains(&first));
        assert_eq!(ids.len(), 2);
    }

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
