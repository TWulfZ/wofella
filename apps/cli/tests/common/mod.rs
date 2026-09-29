//! Shared helpers for the `wolluf` binary tests: a temp data dir and a fixture install built
//! from 002's committed, anonymized DBs (the app's own testkit is `cfg(test)` only).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/dbs");
/// Every environment variable the CLI or the app reads, cleared so the host cannot leak in.
const HOST_ENV: &[&str] = &["WOLLUF_OSU_DIR", "WOLLUF_DATA_DIR", "WOLLUF_LOG"];
/// The fixture scores.db names its one non-empty player `player-01`.
pub(crate) const FIXTURE_PLAYER: &str = "player-01";
/// Distinct natural keys among the five fixture scores (all mode 3).
pub(crate) const FIXTURE_PLAYS: u64 = 5;

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

fn copy(from: &Path, to: &Path) {
    std::fs::copy(from, to).unwrap();
}

pub(crate) fn text(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}
