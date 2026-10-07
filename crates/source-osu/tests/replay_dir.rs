#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;

use wolluf_core::{ChartMd5, FileTime};
use wolluf_source_osu::SourceError;
use wolluf_source_osu::replay_dir::{index, replay_player};

const MD5: &str = "0123456789abcdef0123456789abcdef";
const MD5_B: &str = "fedcba9876543210fedcba9876543210";

#[test]
fn parses_valid_names_ignores_others() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("Data").join("r");
    fs::create_dir_all(&dir).unwrap();
    let upper = MD5.to_uppercase();
    for name in [
        format!("{MD5}-100.osr"),
        format!("{MD5}-100.osg"),
        format!("{MD5}-200.osr"),
        format!("{MD5_B}-5.osg"),
        format!("{upper}-300.osr"),
        format!("{MD5}-400"),
        format!("{MD5}-500.osr.tmp"),
        format!("{MD5}-abc.osr"),
        "notes.txt".to_owned(),
    ] {
        fs::write(dir.join(name), b"x").unwrap();
    }
    fs::create_dir(dir.join(format!("{MD5}-600.osr"))).unwrap();
    let idx = index(root.path()).unwrap();
    let key = |md5: &str, ft: i64| (md5.parse::<ChartMd5>().unwrap(), FileTime::new(ft).unwrap());
    let keys: Vec<_> = idx.keys().copied().collect();
    assert_eq!(keys, vec![key(MD5, 100), key(MD5, 200), key(MD5_B, 5)]);
    let both = &idx[&key(MD5, 100)];
    assert_eq!(
        both.osr.as_deref(),
        Some(dir.join(format!("{MD5}-100.osr")).as_path())
    );
    assert_eq!(
        both.osg.as_deref(),
        Some(dir.join(format!("{MD5}-100.osg")).as_path())
    );
    assert!(idx[&key(MD5, 200)].osg.is_none());
    assert!(idx[&key(MD5_B, 5)].osr.is_none());
}

#[test]
fn missing_dir_is_empty() {
    let root = tempfile::tempdir().unwrap();
    assert!(index(root.path()).unwrap().is_empty());
    fs::create_dir(root.path().join("Data")).unwrap();
    assert!(index(root.path()).unwrap().is_empty());
}

/// The `.osr` header up to the player name; the rest is never read.
fn osr_prefix(player: &[u8]) -> Vec<u8> {
    let mut out = vec![3];
    out.extend_from_slice(&20_260_924_i32.to_le_bytes());
    for s in [MD5.as_bytes(), player] {
        out.push(0x0b);
        out.push(u8::try_from(s.len()).unwrap());
        out.extend_from_slice(s);
    }
    out
}

#[test]
fn replay_player_reads_the_header_name() {
    let dir = tempfile::tempdir().unwrap();
    let osr = dir.path().join("a.osr");
    let mut bytes = osr_prefix(b"TWulfZ");
    bytes.extend(std::iter::repeat_n(0x55, 100_000));
    fs::write(&osr, bytes).unwrap();
    assert_eq!(replay_player(&osr).unwrap(), b"TWulfZ");

    let garbage = dir.path().join("b.osr");
    fs::write(&garbage, [3, 0, 0]).unwrap();
    assert!(matches!(
        replay_player(&garbage),
        Err(SourceError::Codec(_))
    ));
    assert!(matches!(
        replay_player(&dir.path().join("missing.osr")),
        Err(SourceError::Io { .. })
    ));
}
