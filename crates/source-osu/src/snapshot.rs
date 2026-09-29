//! Stable in-memory read of an osu! DB file (spec 003 "Stable read", ADR 0014): osu! may be
//! writing it, so the read is bracketed by two stats and retried with backoff. The file is never
//! copied to disk, which keeps D9's "no fs write" literal.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};
use wolluf_core::BlobSha256;

use crate::error::SourceError;

/// Spec 003: retry after 250 ms, 500 ms and 1 s, then give up with `OSU_RUNNING`.
const DEFAULT_RETRY_DELAYS_MS: [u64; 3] = [250, 500, 1_000];

/// Delays and the sleep function are injectable so tests neither wait nor race.
#[derive(Clone)]
pub struct SnapshotPolicy {
    pub retry_delays: Vec<Duration>,
    pub sleep: Arc<dyn Fn(Duration) + Send + Sync>,
}

impl Default for SnapshotPolicy {
    fn default() -> Self {
        Self {
            retry_delays: DEFAULT_RETRY_DELAYS_MS
                .iter()
                .map(|&ms| Duration::from_millis(ms))
                .collect(),
            sleep: Arc::new(thread::sleep),
        }
    }
}

impl fmt::Debug for SnapshotPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SnapshotPolicy")
            .field("retry_delays", &self.retry_delays)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub bytes: Vec<u8>,
    pub sha256: BlobSha256,
    pub size: u64,
    pub mtime: SystemTime,
}

pub fn read_stable(path: &Path, policy: &SnapshotPolicy) -> Result<Snapshot, SourceError> {
    read_stable_with(path, policy, |p| fs::read(p))
}

/// [`read_stable`] with the read step injected, so a test can simulate osu! writing during the
/// read with real files.
pub fn read_stable_with(
    path: &Path,
    policy: &SnapshotPolicy,
    mut read: impl FnMut(&Path) -> io::Result<Vec<u8>>,
) -> Result<Snapshot, SourceError> {
    let mut delays = policy.retry_delays.iter();
    loop {
        if let Some(snapshot) = attempt(path, &mut read)? {
            return Ok(snapshot);
        }
        match delays.next() {
            Some(&delay) => (policy.sleep)(delay),
            None => {
                return Err(SourceError::Changing {
                    path: PathBuf::from(path),
                });
            }
        }
    }
}

fn stat(path: &Path) -> Result<(u64, SystemTime), SourceError> {
    let meta = fs::metadata(path).map_err(|e| SourceError::io(path, &e))?;
    let mtime = meta.modified().map_err(|e| SourceError::io(path, &e))?;
    Ok((meta.len(), mtime))
}

/// `None` when size or mtime moved during the read, or the bytes read disagree with the size.
fn attempt(
    path: &Path,
    read: &mut impl FnMut(&Path) -> io::Result<Vec<u8>>,
) -> Result<Option<Snapshot>, SourceError> {
    let before = stat(path)?;
    let bytes = read(path).map_err(|e| SourceError::io(path, &e))?;
    let after = stat(path)?;
    let (size, mtime) = after;
    if before != after || bytes.len() as u64 != size {
        return Ok(None);
    }
    let sha256 = BlobSha256(Sha256::digest(&bytes).into());
    Ok(Some(Snapshot {
        bytes,
        sha256,
        size,
        mtime,
    }))
}
