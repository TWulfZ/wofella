//! Reading a chart from `Songs/` and verifying it against the md5 osu!.db recorded (spec 003
//! step 3): a map edited in place must not be archived under the old md5.

use std::fs::{self, File};
use std::io::{self, Read};
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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SongFileError {
    #[error("song file is missing")]
    Missing,
    #[error("song file is {size} bytes, above the {max} byte cap")]
    TooLarge { size: u64, max: u64 },
    #[error("song file read failed: {kind:?}")]
    Io { kind: io::ErrorKind },
}

impl SongFileError {
    /// `TooLarge` is a limit of wolluf, not a request the caller got wrong.
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Missing => ErrorCode::NotFound,
            Self::TooLarge { .. } => ErrorCode::UnsupportedFormat,
            Self::Io { .. } => ErrorCode::Internal,
        }
    }
}

pub(crate) fn stays_inside(path: &Path) -> bool {
    path.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// `rel_path` comes from osu!.db (folder + file). A path that is absolute or climbs out of
/// `songs_dir` cannot name a chart osu! manages, so it is reported as missing instead of read.
pub fn read_chart_verified(
    songs_dir: &Path,
    rel_path: &Path,
    expected_md5: ChartMd5,
) -> Result<Vec<u8>, ChartReadError> {
    if !stays_inside(rel_path) {
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

/// A file of the chart's set folder, such as its `AudioFilename`. `file_name` comes from the
/// `.osu`, which anyone can edit, so it must name one entry of that folder: a name with a
/// separator, `.`/`..` or a drive prefix is reported as missing instead of read.
pub fn read_song_file(
    songs_dir: &Path,
    chart_rel_path: &Path,
    file_name: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, SongFileError> {
    let single_entry = is_single_entry_name(file_name);
    let set_dir = chart_rel_path
        .parent()
        .filter(|dir| stays_inside(dir) && stays_inside(chart_rel_path));
    let (true, Some(set_dir)) = (single_entry, set_dir) else {
        return Err(SongFileError::Missing);
    };
    read_capped(&songs_dir.join(set_dir).join(file_name), max_bytes)
}

/// Names come from files anyone can edit (`.osu`, `skin.ini`, cfg), so they must name one entry
/// of a folder and can never climb out of it once joined.
pub(crate) fn is_single_entry_name(name: &str) -> bool {
    !name.trim().is_empty()
        && !name.contains(['/', '\\', '\0'])
        && matches!(
            Path::new(name).components().collect::<Vec<_>>()[..],
            [Component::Normal(_)]
        )
}

pub(crate) fn read_capped(path: &Path, max_bytes: u64) -> Result<Vec<u8>, SongFileError> {
    // Names Windows cannot open (`<>:"|?*`) are as absent as a missing file.
    let io_error = |e: io::Error| match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::InvalidInput | io::ErrorKind::InvalidFilename => {
            SongFileError::Missing
        }
        kind => SongFileError::Io { kind },
    };
    // Windows refuses to open a directory (PermissionDenied), so a folder named like the wanted
    // file is turned away before the open; size still comes from the handle that is read.
    if !path.is_file() {
        return Err(SongFileError::Missing);
    }
    let file = File::open(path).map_err(io_error)?;
    let meta = file.metadata().map_err(io_error)?;
    if !meta.is_file() {
        return Err(SongFileError::Missing);
    }
    let too_large = |size: u64| SongFileError::TooLarge {
        size,
        max: max_bytes,
    };
    if meta.len() > max_bytes {
        return Err(too_large(meta.len()));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    // The file may grow after the size check; reading one byte past the cap still catches it.
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    let read = bytes.len() as u64;
    if read > max_bytes {
        return Err(too_large(read));
    }
    Ok(bytes)
}
