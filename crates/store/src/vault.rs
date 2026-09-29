//! Content-addressed vault of original `.osr`, `.osg` and `.osu` bytes (architecture §5.2,
//! spec 003). Immutable: decoded forms are derivations, so a decoder bug is always fixable.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use wolluf_core::BlobSha256;

use crate::error::StoreError;

const BLOBS_DIR: &str = "blobs";
const TMP_DIR: &str = "tmp";

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

pub fn sha256(bytes: &[u8]) -> BlobSha256 {
    BlobSha256(Sha256::digest(bytes).into())
}

impl Vault {
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        for dir in [root.join(BLOBS_DIR), root.join(TMP_DIR)] {
            std::fs::create_dir_all(&dir).map_err(|e| StoreError::io(dir, e))?;
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// Durable before it returns, so a `blob` row inserted afterwards never points at a file a
    /// crash could still lose. A crash before the row leaves an unreferenced file, which is
    /// harmless (no GC in F0).
    pub fn put(&self, bytes: &[u8]) -> Result<BlobSha256, StoreError> {
        self.put_with(bytes, |file, bytes| file.write_all(bytes))
    }

    fn put_with(
        &self,
        bytes: &[u8],
        write: impl FnOnce(&mut File, &[u8]) -> std::io::Result<()>,
    ) -> Result<BlobSha256, StoreError> {
        let sha = sha256(bytes);
        let target = self.path(sha);
        // Same address and same size means same content; a shorter file is a torn copy from
        // outside the vault's own atomic path and gets replaced.
        if std::fs::metadata(&target).is_ok_and(|m| m.len() == bytes.len() as u64) {
            return Ok(sha);
        }

        let tmp = self
            .root
            .join(TMP_DIR)
            .join(ulid::Ulid::generate().to_string());
        let written = write_synced(&tmp, bytes, write).and_then(|()| {
            let parent = target.parent().unwrap_or(&self.root);
            std::fs::create_dir_all(parent)?;
            std::fs::rename(&tmp, &target)?;
            sync_dir(parent)
        });
        written.map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            StoreError::io(&target, e)
        })?;
        Ok(sha)
    }

    /// Re-hashes on every read: the vault is the only copy once osu! drops the original.
    pub fn get(&self, sha: BlobSha256) -> Result<Vec<u8>, StoreError> {
        let path = self.path(sha);
        let bytes = std::fs::read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => StoreError::BlobMissing(sha),
            _ => StoreError::io(&path, e),
        })?;
        if sha256(&bytes) != sha {
            return Err(StoreError::VaultCorrupt(sha));
        }
        Ok(bytes)
    }

    /// `blobs/<h[0..2]>/<h[2..4]>/<h>`: two fan-out levels keep directories small at ~10k blobs.
    pub fn path(&self, sha: BlobSha256) -> PathBuf {
        let hex = sha.to_string();
        self.root
            .join(BLOBS_DIR)
            .join(&hex[0..2])
            .join(&hex[2..4])
            .join(&hex)
    }

    pub fn contains(&self, sha: BlobSha256) -> bool {
        self.path(sha).is_file()
    }
}

fn write_synced(
    path: &Path,
    bytes: &[u8],
    write: impl FnOnce(&mut File, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut file = File::create_new(path)?;
    write(&mut file, bytes)?;
    file.sync_all()
}

/// The rename is only durable once the directory entry is flushed (POSIX). Windows has no
/// directory handle to fsync; NTFS journals the rename itself.
#[cfg(unix)]
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BYTES: &[u8] = b"osu! replay bytes";

    fn tmp_entries(vault_root: &Path) -> usize {
        std::fs::read_dir(vault_root.join(TMP_DIR)).unwrap().count()
    }

    #[test]
    fn put_layout_ab_cd_sha() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        let sha = vault.put(BYTES).unwrap();
        assert_eq!(sha, sha256(BYTES));
        let hex = sha.to_string();
        let expected = dir
            .path()
            .join(BLOBS_DIR)
            .join(&hex[0..2])
            .join(&hex[2..4])
            .join(&hex);
        assert_eq!(vault.path(sha), expected);
        assert_eq!(std::fs::read(&expected).unwrap(), BYTES);
        assert_eq!(tmp_entries(dir.path()), 0);
    }

    #[test]
    fn put_twice_same_path_no_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        let sha = vault.put(BYTES).unwrap();
        let first = std::fs::metadata(vault.path(sha)).unwrap();
        assert_eq!(vault.put(BYTES).unwrap(), sha);
        let second = std::fs::metadata(vault.path(sha)).unwrap();
        assert_eq!(first.modified().unwrap(), second.modified().unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // A rewrite goes through rename, which would give the path a new inode.
            assert_eq!(first.ino(), second.ino());
        }
        assert_eq!(tmp_entries(dir.path()), 0);
    }

    #[test]
    fn no_partial_file_after_failed_write() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        let res = vault.put_with(BYTES, |file, bytes| {
            file.write_all(&bytes[..bytes.len() / 2])?;
            Err(std::io::Error::other("disk full"))
        });
        assert!(matches!(res, Err(StoreError::Io { .. })));
        assert!(!vault.path(sha256(BYTES)).exists());
        assert_eq!(tmp_entries(dir.path()), 0);
        // The failed attempt does not block a later good write.
        assert_eq!(vault.get(vault.put(BYTES).unwrap()).unwrap(), BYTES);
    }

    #[test]
    fn get_detects_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        let sha = vault.put(BYTES).unwrap();
        assert_eq!(vault.get(sha).unwrap(), BYTES);
        std::fs::write(vault.path(sha), b"flipped bits").unwrap();
        assert!(matches!(vault.get(sha), Err(StoreError::VaultCorrupt(s)) if s == sha));
        let absent = sha256(b"never stored");
        assert!(matches!(vault.get(absent), Err(StoreError::BlobMissing(s)) if s == absent));
    }

    #[test]
    fn put_repairs_a_truncated_final_file() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path()).unwrap();
        let sha = vault.put(BYTES).unwrap();
        std::fs::write(vault.path(sha), &BYTES[..3]).unwrap();
        vault.put(BYTES).unwrap();
        assert_eq!(vault.get(sha).unwrap(), BYTES);
    }
}
