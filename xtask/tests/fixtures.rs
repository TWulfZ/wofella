#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Goldens and anonymization checks over the committed fixtures (spec 002 AC13). They live in
//! xtask, next to the generator, because the Stage-4 lane that wrote them owns `xtask/` and
//! `fixtures/` but not `crates/source-osu/tests/`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};
use wolluf_source_osu::codec::collection_db::{decode_collection_db, encode_collection_db};
use wolluf_source_osu::codec::osr::{check_name_consistency, decode_osr};
use wolluf_source_osu::codec::osu_db::decode_osu_db;
use wolluf_source_osu::codec::replay_name::ReplayFileName;
use wolluf_source_osu::codec::scores_db::decode_scores_db;
use wolluf_source_osu::testkit::{encode_osu_db, encode_scores_db};

const MD5_HEX_LEN: usize = 32;
const MAX_FIXTURE_MD5: usize = 1000;
/// Pilot names the specs already publish (CLAUDE.md, spec 002 R9); other players' real names must
/// not be written into a test either, so the generator's leak check covers them.
const DENYLISTED_NAMES: &[&str] = &["twulfz", "wulfz"];

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures")
}

fn only_file(dir: &str, suffix: &str) -> Vec<u8> {
    let dir = fixtures_dir().join(dir);
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}; run `cargo xtask fixtures`", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_str().is_some_and(|s| s.ends_with(suffix)))
        .collect();
    assert_eq!(files.len(), 1, "exactly one {suffix} in {}", dir.display());
    fs::read(files.remove(0)).unwrap()
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out.sort();
    out
}

#[test]
fn osu_db_fixture_golden() {
    let bytes = only_file("dbs/osu_db", ".min.db");
    let (db, diags) = decode_osu_db(&bytes).unwrap();
    assert_eq!(
        encode_osu_db(&db),
        bytes,
        "decode/encode must be byte-identical"
    );
    assert_eq!(db.beatmaps.len(), 12);
    let modes: BTreeSet<u8> = db.beatmaps.iter().map(|b| b.mode).collect();
    assert_eq!(modes, BTreeSet::from([0, 1, 2, 3]));
    let keymodes: BTreeSet<u8> = db
        .beatmaps
        .iter()
        .filter_map(|b| b.mania_keymode().map(|k| k.columns()))
        .collect();
    assert!(
        keymodes.contains(&4) && keymodes.contains(&7),
        "{keymodes:?}"
    );
    assert!(db.beatmaps.iter().any(|b| !b.timing_points.is_empty()));
    insta::assert_debug_snapshot!((db, diags));
}

#[test]
fn scores_db_fixture_golden() {
    let bytes = only_file("dbs/scores_db", ".min.db");
    let (db, diags) = decode_scores_db(&bytes).unwrap();
    assert_eq!(
        encode_scores_db(&db),
        bytes,
        "decode/encode must be byte-identical"
    );
    assert!(db.scores().count() <= 40);
    assert!(db.scores().any(|s| s.header.player.as_bytes() == Some(&[])));
    assert!(db.scores().any(|s| s.header.is_score_v2()));
    assert!(db.scores().any(|s| s.online_id.positive().is_some()));
    let (osu, _) = decode_osu_db(&only_file("dbs/osu_db", ".min.db")).unwrap();
    for group in &db.beatmaps {
        assert!(
            osu.beatmaps.iter().any(|b| b.md5 == group.md5),
            "every scored md5 has its osu!.db entry"
        );
    }
    insta::assert_debug_snapshot!((db, diags));
}

#[test]
fn collection_db_fixture_golden() {
    let bytes = only_file("dbs/collection_db", ".min.db");
    let (db, diags) = decode_collection_db(&bytes).unwrap();
    assert_eq!(
        encode_collection_db(&db),
        bytes,
        "decode/encode must be byte-identical"
    );
    assert_eq!(db.collections.len(), 2);
    insta::assert_debug_snapshot!((db, diags));
}

#[test]
fn synthetic_osr_goldens() {
    let dir = fixtures_dir().join("synthetic/osr");
    let files = files_under(&dir);
    assert_eq!(files.len(), 4, "run `cargo xtask fixtures synthetic`");
    let decoded: Vec<_> = files
        .iter()
        .map(|path| {
            let name = path.file_name().unwrap().to_str().unwrap().to_owned();
            let parsed = ReplayFileName::parse(&name).expect("Data/r name");
            let (osr, diags) = decode_osr(&fs::read(path).unwrap()).unwrap();
            let name_diags = check_name_consistency(&parsed, &osr.header);
            assert!(name_diags.is_empty(), "{name}: {name_diags:?}");
            (name, osr, diags)
        })
        .collect();
    insta::assert_debug_snapshot!(decoded);
}

fn fixture_md5s() -> BTreeSet<Vec<u8>> {
    (0..MAX_FIXTURE_MD5)
        .map(|n| {
            Md5::digest(format!("wolluf-fixture:{n}"))
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
                .into_bytes()
        })
        .collect()
}

/// Maximal runs of lowercase hex digits, with their offsets.
fn hex_runs(bytes: &[u8]) -> Vec<(usize, &[u8])> {
    let is_hex = |b: &u8| b.is_ascii_digit() || (b'a'..=b'f').contains(b);
    let mut runs = Vec::new();
    let mut start = None;
    for (i, b) in bytes.iter().chain([&0]).enumerate() {
        match (is_hex(b), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                runs.push((s, &bytes[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    runs
}

#[test]
fn fixtures_are_anonymized() {
    let allowed = fixture_md5s();
    let mut files = files_under(&fixtures_dir().join("dbs"));
    files.extend(files_under(&fixtures_dir().join("synthetic")));
    assert!(files.len() >= 8, "{files:?}");
    for path in files {
        let bytes = fs::read(&path).unwrap();
        let lower = bytes.to_ascii_lowercase();
        for name in DENYLISTED_NAMES {
            assert!(
                !lower.windows(name.len()).any(|w| w == name.as_bytes()),
                "{}: contains a denylisted pilot name",
                path.display()
            );
        }
        // The manifest's sha256 values are 64-hex by design; it holds no md5 fields.
        if path.file_name().is_some_and(|n| n == "MANIFEST.toml") {
            continue;
        }
        let mut names = vec![
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .as_bytes()
                .to_vec(),
        ];
        names.push(bytes);
        for haystack in &names {
            for (offset, run) in hex_runs(haystack) {
                if run.len() < MD5_HEX_LEN {
                    continue;
                }
                // An osu! String is preceded by its length byte (0x20, not hex), but the next binary
                // field may start with a byte in the hex range and extend the run past 32.
                assert!(
                    run.len() < 2 * MD5_HEX_LEN && allowed.contains(&run[..MD5_HEX_LEN]),
                    "{}: hex run of {} chars at {offset} is not md5(\"wolluf-fixture:<n>\")",
                    path.display(),
                    run.len()
                );
            }
        }
    }
}
