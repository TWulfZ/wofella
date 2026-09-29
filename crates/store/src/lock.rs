use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use crate::error::StoreError;

/// Held for the process lifetime so two wolluf processes never share a data dir (spec 003). The
/// OS releases the lock when the process dies, so a crash never leaves a stale lock behind.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
    path: PathBuf,
}

impl InstanceLock {
    pub fn acquire(path: &Path) -> Result<Self, StoreError> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(|e| StoreError::io(path, e))?;
        match file.try_lock() {
            Ok(()) => Ok(Self {
                _file: file,
                path: path.to_path_buf(),
            }),
            Err(TryLockError::WouldBlock) => Err(StoreError::InstanceLocked(path.to_path_buf())),
            Err(TryLockError::Error(e)) => Err(StoreError::io(path, e)),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_instance_lock_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wolluf.lock");
        let first = InstanceLock::acquire(&path).unwrap();
        assert!(matches!(
            InstanceLock::acquire(&path),
            Err(StoreError::InstanceLocked(_))
        ));
        drop(first);
        InstanceLock::acquire(&path).unwrap();
    }
}
