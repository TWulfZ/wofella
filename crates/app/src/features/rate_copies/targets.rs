//! What already holds the copy's names in the set folder. Read-only: writes go through
//! `app::export`.

use std::fs::File;
use std::io::{self, Read};

use super::assess::audio_filename;
use crate::errors::AppError;
use crate::export::SetFolder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OsuTarget {
    Free,
    /// A `.osu` that plays the copy's audio: an earlier copy, skipped.
    SameCopy,
    /// Anything else; the copy cannot be written under that name.
    Taken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AudioTarget {
    Free,
    Reusable,
    Unusable,
}

/// Enough for the Ogg page header and the Vorbis identification packet that opens the stream.
const AUDIO_PROBE_BYTES: u64 = 64;

pub(super) fn osu_target(
    folder: &SetFolder,
    osu_filename: &str,
    audio_filename_of_copy: &str,
    max_bytes: u64,
) -> Result<OsuTarget, AppError> {
    let path = folder.entry(osu_filename)?;
    let io = |e: io::Error| AppError::internal(format!("inspect {}: {e}", path.display()));
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(OsuTarget::Free),
        Err(e) => return Err(io(e)),
    };
    if !meta.file_type().is_file() || meta.len() > max_bytes {
        return Ok(OsuTarget::Taken);
    }
    let mut osu = Vec::new();
    File::open(&path)
        .and_then(|f| f.take(max_bytes).read_to_end(&mut osu))
        .map_err(io)?;
    Ok(
        if audio_filename(&osu).as_deref() == Some(audio_filename_of_copy) {
            OsuTarget::SameCopy
        } else {
            OsuTarget::Taken
        },
    )
}

/// A file at the audio name is reused only when it is a regular, non-empty Ogg Vorbis file: a
/// stub or a folder there would leave the copy silent in stable.
pub(super) fn audio_target(
    folder: &SetFolder,
    audio_filename: &str,
) -> Result<AudioTarget, AppError> {
    let path = folder.entry(audio_filename)?;
    let io = |e: io::Error| AppError::internal(format!("inspect {}: {e}", path.display()));
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(AudioTarget::Free),
        Err(e) => return Err(io(e)),
    };
    if !meta.file_type().is_file() || meta.len() == 0 {
        return Ok(AudioTarget::Unusable);
    }
    let mut head = Vec::new();
    File::open(&path)
        .and_then(|f| f.take(AUDIO_PROBE_BYTES).read_to_end(&mut head))
        .map_err(io)?;
    let vorbis = head.starts_with(b"OggS") && head.windows(7).any(|w| w == b"\x01vorbis");
    Ok(if vorbis {
        AudioTarget::Reusable
    } else {
        AudioTarget::Unusable
    })
}
