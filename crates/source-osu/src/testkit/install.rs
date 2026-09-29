use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::codec::replay_name::ReplayFileName;
use crate::testkit::{OsuDbBuilder, ScoresDbBuilder};

/// Never a runnable binary: only the name matters to validation.
const EXE_STUB: &[u8] = b"MZ wolluf fake osu!.exe";
const DEFAULT_ACCOUNT: &str = "fixture";
const DEFAULT_CFG: &[u8] =
    b"# fake cfg\r\nUsername = Rosalind\r\nPassword = WOLLUF_SENTINEL_9f3a\r\n";

/// A fake osu! stable tree as a relative-path → bytes map. The crate never writes files (D9),
/// so tests materialise the map themselves, under `tests/`.
#[derive(Debug, Clone)]
pub struct FakeInstall {
    files: BTreeMap<PathBuf, Vec<u8>>,
}

impl Default for FakeInstall {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeInstall {
    /// osu!.exe stub, an empty osu!.db and scores.db at the newest verified builds, and one
    /// account cfg.
    pub fn new() -> Self {
        Self::empty()
            .file("osu!.exe", EXE_STUB.to_vec())
            .osu_db(OsuDbBuilder::new().encode())
            .scores_db(ScoresDbBuilder::new().encode())
            .cfg(DEFAULT_ACCOUNT, DEFAULT_CFG)
    }

    pub const fn empty() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    /// A lazer data folder: `client.realm` and no osu!.db.
    pub fn lazer() -> Self {
        Self::empty().file("client.realm", b"realm".to_vec())
    }

    pub fn file(mut self, rel: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) -> Self {
        self.files.insert(rel.into(), bytes.into());
        self
    }

    pub fn without(mut self, rel: impl Into<PathBuf>) -> Self {
        self.files.remove(&rel.into());
        self
    }

    pub fn osu_db(self, bytes: Vec<u8>) -> Self {
        self.file("osu!.db", bytes)
    }

    pub fn scores_db(self, bytes: Vec<u8>) -> Self {
        self.file("scores.db", bytes)
    }

    pub fn collection_db(self, bytes: Vec<u8>) -> Self {
        self.file("collection.db", bytes)
    }

    pub fn cfg(self, account: &str, contents: impl Into<Vec<u8>>) -> Self {
        self.file(format!("osu!.{account}.cfg"), contents)
    }

    pub fn replay(self, name: &ReplayFileName, bytes: Vec<u8>) -> Self {
        self.file(PathBuf::from("Data").join("r").join(name.format()), bytes)
    }

    /// `rel` is relative to `Songs/`.
    pub fn song(self, rel: impl Into<PathBuf>, bytes: Vec<u8>) -> Self {
        self.file(PathBuf::from("Songs").join(rel.into()), bytes)
    }

    pub fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        self.files.clone()
    }
}
