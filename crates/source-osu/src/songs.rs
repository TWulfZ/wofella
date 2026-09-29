//! Reading a chart from `Songs/` and verifying it against the md5 osu!.db recorded (spec 003
//! step 3): a map edited in place must not be archived under the old md5.

use std::fs;
use std::io;
use std::path::{Component, Path};

use md5::{Digest, Md5};
use thiserror::Error;
use wolluf_core::{ChartMd5, ErrorCode};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChartReadError {
    #[error("chart file is missing")]
    Missing,
    #[error("chart md5 is {actual}, osu!.db expects {expected}")]
    Md5Mismatch {
        expected: ChartMd5,
        actual: ChartMd5,
    },
    #[error("chart read failed: {kind:?}")]
    Io { kind: io::ErrorKind },
}

impl ChartReadError {
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Missing => ErrorCode::NotFound,
            Self::Md5Mismatch { .. } => ErrorCode::Conflict,
            Self::Io { .. } => ErrorCode::Internal,
        }
    }
}

/// `rel_path` comes from osu!.db (folder + file). A path that is absolute or climbs out of
/// `songs_dir` cannot name a chart osu! manages, so it is reported as missing instead of read.
pub fn read_chart_verified(
    songs_dir: &Path,
    rel_path: &Path,
    expected_md5: ChartMd5,
) -> Result<Vec<u8>, ChartReadError> {
    if !rel_path
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(ChartReadError::Missing);
    }
    let bytes = fs::read(songs_dir.join(rel_path)).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => ChartReadError::Missing,
        kind => ChartReadError::Io { kind },
    })?;
    let actual = ChartMd5(Md5::digest(&bytes).into());
    if actual == expected_md5 {
        Ok(bytes)
    } else {
        Err(ChartReadError::Md5Mismatch {
            expected: expected_md5,
            actual,
        })
    }
}
