//! Shared helpers for the `wolluf` binary tests: a temp data dir and a fixture install built
//! from 002's committed, anonymized DBs (the app's own testkit is `cfg(test)` only).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use md5::{Digest, Md5};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/dbs");
/// Every environment variable the CLI or the app reads, cleared so the host cannot leak in.
const HOST_ENV: &[&str] = &["WOLLUF_OSU_DIR", "WOLLUF_DATA_DIR", "WOLLUF_LOG"];
/// The fixture scores.db names its one non-empty player `player-01`.
pub(crate) const FIXTURE_PLAYER: &str = "player-01";
/// Distinct natural keys among the five fixture scores (all mode 3).
pub(crate) const FIXTURE_PLAYS: u64 = 5;
/// Fixture osu!.db entry 1: 7K mania, no scores. Its md5 is `md5("wolluf-fixture:1")`
/// (fixtures/dbs/MANIFEST.toml), its catalog path `folder-1/file-1`.
const FIXTURE_7K_MD5: &str = "643833896a402cef06fd6ee5c120211a";
const FIXTURE_7K_FOLDER: &str = "folder-1";
const FIXTURE_7K_FILE: &str = "file-1";
pub(crate) const FIXTURE_7K_TITLE: &str = "title-1";

pub(crate) struct Env {
    pub(crate) dir: tempfile::TempDir,
}

impl Env {
    pub(crate) fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    pub(crate) fn data_dir(&self) -> PathBuf {
        self.dir.path().join("data")
    }

    /// A valid stable install: stub exe, the minimized osu!.db and scores.db, and a cfg whose
    /// login is the fixture player.
    pub(crate) fn install(&self) -> PathBuf {
        let root = self.dir.path().join("osu!");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("osu!.exe"), b"MZ").unwrap();
        copy(
            &Path::new(FIXTURES).join("osu_db/osu-20260924.min.db"),
            &root.join("osu!.db"),
        );
        copy(
            &Path::new(FIXTURES).join("scores_db/scores-20260924.min.db"),
            &root.join("scores.db"),
        );
        std::fs::write(
            root.join("osu!.fixture.cfg"),
            format!("Username = {FIXTURE_PLAYER}\n"),
        )
        .unwrap();
        root
    }

    /// [`Self::install`] whose 7K entry 1 points at `osu` in `Songs/`. The CLI may not depend on
    /// source-osu's testkit (layers.toml), so the fixture osu!.db is patched in place: an md5
    /// has a fixed length, so swapping it keeps the file valid. Returns the root and the md5.
    pub(crate) fn install_with_chart(&self, osu: &[u8]) -> (PathBuf, String) {
        let root = self.install();
        let md5 = hex(&Md5::digest(osu));
        let db_path = root.join("osu!.db");
        let mut db = std::fs::read(&db_path).unwrap();
        let at = db
            .windows(FIXTURE_7K_MD5.len())
            .position(|w| w == FIXTURE_7K_MD5.as_bytes())
            .unwrap();
        db[at..at + md5.len()].copy_from_slice(md5.as_bytes());
        std::fs::write(&db_path, db).unwrap();
        let folder = root.join("Songs").join(FIXTURE_7K_FOLDER);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(FIXTURE_7K_FILE), osu).unwrap();
        (root, md5)
    }

    pub(crate) fn wolluf(&self) -> Command {
        let mut cmd = Command::cargo_bin("wolluf").unwrap();
        for var in HOST_ENV {
            cmd.env_remove(var);
        }
        cmd.arg("--data-dir").arg(self.data_dir());
        cmd
    }

    pub(crate) fn json(&self, args: &[&str]) -> serde_json::Value {
        let out = self.wolluf().arg("--json").args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {:?}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }

    pub(crate) fn set_install(&self, root: &Path) {
        self.wolluf()
            .args(["setup", "set"])
            .arg(root)
            .assert()
            .success();
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A v14 osu!mania file. `taps` are `(column, ms)`, `holds` `(column, head ms, tail ms)`.
pub(crate) fn osu_7k(taps: &[(u8, i32)], holds: &[(u8, i32, i32)]) -> Vec<u8> {
    const KEYS: u32 = 7;
    let x = |col: u8| (2 * u32::from(col) + 1) * 256 / KEYS;
    let mut text = format!(
        "osu file format v14\n\n[General]\nAudioFilename: audio.mp3\nMode: 3\n\n\
         [Metadata]\nTitle: t\nArtist: a\nCreator: c\nVersion: v\n\n\
         [Difficulty]\nHPDrainRate: 8\nCircleSize: {KEYS}\nOverallDifficulty: 8\n\n\
         [TimingPoints]\n0,500,4,1,0,100,1,0\n\n[HitObjects]\n"
    );
    for (col, t) in taps {
        text.push_str(&format!("{},192,{t},1,0,0:0:0:0:\n", x(*col)));
    }
    for (col, head, tail) in holds {
        text.push_str(&format!("{},192,{head},128,0,{tail}:0:0:0:0:\n", x(*col)));
    }
    text.into_bytes()
}

fn copy(from: &Path, to: &Path) {
    std::fs::copy(from, to).unwrap();
}

pub(crate) fn text(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}
