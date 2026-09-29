#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use md5::{Digest, Md5};
use wolluf_core::{ChartMd5, ErrorCode};
use wolluf_source_osu::songs::{ChartReadError, read_chart_verified};

fn md5_of(bytes: &[u8]) -> ChartMd5 {
    ChartMd5(Md5::digest(bytes).into())
}

#[test]
fn md5_mismatch_reported() {
    let songs = tempfile::tempdir().unwrap();
    fs::create_dir(songs.path().join("123 Artist - Title")).unwrap();
    let rel = Path::new("123 Artist - Title").join("map [7K].osu");
    fs::write(songs.path().join(&rel), b"osu file format v14\n").unwrap();
    let good = md5_of(b"osu file format v14\n");
    assert_eq!(
        read_chart_verified(songs.path(), &rel, good).unwrap(),
        b"osu file format v14\n"
    );
    let edited = md5_of(b"original bytes");
    let err = read_chart_verified(songs.path(), &rel, edited).unwrap_err();
    assert_eq!(
        err,
        ChartReadError::Md5Mismatch {
            expected: edited,
            actual: good
        }
    );
    assert_eq!(err.code(), ErrorCode::Conflict);
}

#[test]
fn missing_chart_reported() {
    let songs = tempfile::tempdir().unwrap();
    let md5 = md5_of(b"x");
    let err = read_chart_verified(songs.path(), Path::new("gone/map.osu"), md5).unwrap_err();
    assert_eq!(err, ChartReadError::Missing);
    assert_eq!(err.code(), ErrorCode::NotFound);
    // osu!.db paths never climb out of Songs; such a path is treated as missing, not read.
    fs::write(songs.path().join("outside.osu"), b"x").unwrap();
    let inner = songs.path().join("sub");
    fs::create_dir(&inner).unwrap();
    assert_eq!(
        read_chart_verified(&inner, Path::new("../outside.osu"), md5).unwrap_err(),
        ChartReadError::Missing
    );
}
