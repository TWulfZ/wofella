//! `Data/r` index (spec 003). Names go through 002's `ReplayFileName::parse`, never a second
//! pattern.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use wolluf_core::{ChartMd5, FileTime};

use crate::codec::replay_name::{ReplayFileKind, ReplayFileName};
use crate::codec::score_header::read_player;
use crate::error::SourceError;

const DATA_DIR: &str = "Data";
const REPLAY_DIR: &str = "r";
/// Mode, version, the md5 string and a player name fit many times over; the life bar that
/// follows can be kilobytes long and is never read.
const PLAYER_PREFIX_BYTES: u64 = 4_096;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayFiles {
    pub osr: Option<PathBuf>,
    pub osg: Option<PathBuf>,
}

/// `BTreeMap` so every consumer walks plays in the same order (D3). A missing `Data/r` is an
/// empty index: fresh installs have none.
pub fn index(root: &Path) -> Result<BTreeMap<(ChartMd5, FileTime), ReplayFiles>, SourceError> {
    let dir = root.join(DATA_DIR).join(REPLAY_DIR);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(SourceError::io(&dir, &e)),
    };
    let mut out: BTreeMap<(ChartMd5, FileTime), ReplayFiles> = BTreeMap::new();
    for entry in entries {
        let entry = entry.map_err(|e| SourceError::io(&dir, &e))?;
        let Some(name) = entry.file_name().to_str().and_then(ReplayFileName::parse) else {
            continue;
        };
        let is_file = entry
            .file_type()
            .map_err(|e| SourceError::io(entry.path(), &e))?
            .is_file();
        if !is_file {
            continue;
        }
        let slot = out.entry((name.md5, name.filetime)).or_default();
        match name.kind {
            ReplayFileKind::Osr => slot.osr = Some(entry.path()),
            ReplayFileKind::Osg => slot.osg = Some(entry.path()),
        }
    }
    Ok(out)
}

/// The player name in a `.osr` header, read from the file's first bytes only. An absent name is
/// empty, as sync stores it.
pub fn replay_player(path: &Path) -> Result<Vec<u8>, SourceError> {
    let file = fs::File::open(path).map_err(|e| SourceError::io(path, &e))?;
    let mut prefix = Vec::new();
    file.take(PLAYER_PREFIX_BYTES)
        .read_to_end(&mut prefix)
        .map_err(|e| SourceError::io(path, &e))?;
    Ok(read_player(&prefix)?.bytes_or_empty().to_vec())
}
