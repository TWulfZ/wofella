//! An image of a chart's set folder, such as its `[Events]` background, read in place and
//! described by its bytes (ADR 0018, ADR 0019).

use std::io;
use std::path::Path;

use thiserror::Error;
use wolluf_core::ErrorCode;

use crate::skins::{ImageKind, list_dir, matching, sniff};
use crate::songs::{SongFileError, is_single_entry_name, read_capped, stays_inside};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SongImageLimits {
    /// Above this the file is refused before it is read.
    pub max_bytes: u64,
    /// Entries listed per folder while matching a name case-insensitively.
    pub max_dir_entries: usize,
    /// Path components a reference may have, file name included.
    pub max_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongImage {
    /// The reference resolved to its on-disk spelling, relative to the set folder, `/`-separated.
    pub path: String,
    pub kind: ImageKind,
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SongImageError {
    #[error("song image is missing")]
    Missing,
    #[error("song image is {size} bytes, above the {max} byte cap")]
    TooLarge { size: u64, max: u64 },
    #[error("song image is not a PNG or JPEG the webview decodes")]
    Unsupported,
    #[error("song image read failed: {kind:?}")]
    Io { kind: io::ErrorKind },
}

impl SongImageError {
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Missing => ErrorCode::NotFound,
            Self::TooLarge { .. } | Self::Unsupported => ErrorCode::UnsupportedFormat,
            Self::Io { .. } => ErrorCode::Internal,
        }
    }
}

const SEPARATOR: char = '/';

/// `reference` comes from the `.osu`, which anyone can edit: each component must name one entry
/// and none may hold `:` (a drive prefix or an alternate data stream on Windows), so it cannot
/// leave the set folder. Components match as on NTFS, the exact spelling first.
pub fn read_song_image(
    songs_dir: &Path,
    chart_rel_path: &Path,
    reference: &str,
    limits: &SongImageLimits,
) -> Result<SongImage, SongImageError> {
    let set_dir = chart_rel_path
        .parent()
        .filter(|dir| stays_inside(dir) && stays_inside(chart_rel_path))
        .ok_or(SongImageError::Missing)?;
    let normalized = reference.replace('\\', "/");
    let parts: Vec<&str> = normalized.split(SEPARATOR).collect();
    let safe = parts
        .iter()
        .all(|p| is_single_entry_name(p) && !p.contains(':'));
    let Some((&file, dirs)) = parts.split_last() else {
        return Err(SongImageError::Missing);
    };
    if !safe || parts.len() > limits.max_depth {
        return Err(SongImageError::Missing);
    }
    let mut path = songs_dir.join(set_dir);
    let mut spelled: Vec<String> = Vec::with_capacity(parts.len());
    for (name, want_dir) in dirs.iter().map(|d| (*d, true)).chain([(file, false)]) {
        let (entries, _) =
            list_dir(&path, limits.max_dir_entries).map_err(|_| SongImageError::Missing)?;
        let found = matching(&entries, name).into_iter().find(|candidate| {
            let p = path.join(candidate);
            if want_dir { p.is_dir() } else { p.is_file() }
        });
        let found = found.ok_or(SongImageError::Missing)?.to_owned();
        path.push(&found);
        spelled.push(found);
    }
    let bytes = read_capped(&path, limits.max_bytes).map_err(|e| match e {
        SongFileError::Missing => SongImageError::Missing,
        SongFileError::TooLarge { size, max } => SongImageError::TooLarge { size, max },
        SongFileError::Io { kind } => SongImageError::Io { kind },
    })?;
    let (kind, width, height) = sniff(&bytes).map_err(|_| SongImageError::Unsupported)?;
    Ok(SongImage {
        path: spelled.join("/"),
        kind,
        width,
        height,
        bytes,
    })
}
