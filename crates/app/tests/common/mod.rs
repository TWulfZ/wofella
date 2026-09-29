//! Shared by the `#[ignore]` corpus tests: locating `WOLLUF_CORPUS` and proving it was only read.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const CORPUS_ENV: &str = "WOLLUF_CORPUS";
const OSU_DB: &str = "osu!.db";
/// Walking every beatmap folder on drvfs takes minutes. The folders themselves are recorded:
/// creating, deleting or renaming a file inside one changes that folder's mtime.
const SONGS_DIR: &str = "Songs";

pub(crate) fn corpus() -> PathBuf {
    let Some(raw) = std::env::var_os(CORPUS_ENV) else {
        panic!(
            "{CORPUS_ENV} is unset; corpus tests need an osu! stable install, e.g. \
             {CORPUS_ENV}=\"/mnt/e/Games/osu!\" cargo nextest run -p wolluf-app --run-ignored only"
        );
    };
    let root = PathBuf::from(raw);
    assert!(
        root.join(OSU_DB).is_file(),
        "{CORPUS_ENV}={} has no {OSU_DB}",
        root.display()
    );
    root
}

pub(crate) type TreeState = BTreeMap<PathBuf, (u64, Option<SystemTime>)>;

/// Length and mtime of every entry under `root`, without following symlinks.
pub(crate) fn tree_state(root: &Path) -> TreeState {
    fn walk(root: &Path, dir: &Path, out: &mut TreeState) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            out.insert(path.clone(), (meta.len(), meta.modified().ok()));
            if meta.is_dir() && dir != root.join(SONGS_DIR) {
                walk(root, &path, out);
            }
        }
    }
    let mut out = TreeState::new();
    walk(root, root, &mut out);
    out
}

pub(crate) fn assert_unchanged(during: &str, before: &TreeState, after: &TreeState) {
    let changed: Vec<&PathBuf> = before
        .keys()
        .chain(after.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|p| before.get(*p) != after.get(*p))
        .collect();
    assert!(
        changed.is_empty(),
        "entries under {CORPUS_ENV} changed during {during} (is osu! running?): {changed:?}"
    );
}
