#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use md5::{Digest, Md5};
use wolluf_core::{ChartMd5, ErrorCode};
use wolluf_source_osu::songs::{
    ChartReadError, SongFileError, read_chart_verified, read_song_file,
};

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

const CAP: u64 = 1_024;

/// `Songs/<set>/map.osu` plus `Songs/<set>/audio.mp3`, and `Songs/secret.mp3` beside the set.
fn song_tree() -> (tempfile::TempDir, std::path::PathBuf) {
    let songs = tempfile::tempdir().unwrap();
    let set = songs.path().join("123 Artist - Title");
    fs::create_dir(&set).unwrap();
    fs::write(set.join("map [7K].osu"), b"osu file format v14\n").unwrap();
    fs::write(set.join("audio.mp3"), b"ID3 audio").unwrap();
    fs::write(songs.path().join("secret.mp3"), b"outside the set").unwrap();
    let chart = Path::new("123 Artist - Title").join("map [7K].osu");
    (songs, chart)
}

#[test]
fn read_song_file_reads_next_to_the_chart() {
    let (songs, chart) = song_tree();
    assert_eq!(
        read_song_file(songs.path(), &chart, "audio.mp3", CAP).unwrap(),
        b"ID3 audio"
    );
    let nested = songs.path().join("Normal").join("9 Set");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("song.ogg"), b"OggS").unwrap();
    assert_eq!(
        read_song_file(
            songs.path(),
            Path::new("Normal/9 Set/x.osu"),
            "song.ogg",
            CAP
        )
        .unwrap(),
        b"OggS",
        "nested Songs folders"
    );
}

#[test]
fn read_song_file_rejects_names_that_leave_the_set_folder() {
    let (songs, chart) = song_tree();
    let absolute = songs.path().join("secret.mp3");
    for name in [
        "../secret.mp3",
        "..",
        ".",
        absolute.to_str().unwrap(),
        "sub/audio.mp3",
        "sub\\audio.mp3",
        "",
        "   ",
        "a\0b.mp3",
    ] {
        let err = read_song_file(songs.path(), &chart, name, CAP).unwrap_err();
        assert_eq!(err, SongFileError::Missing, "{name:?}");
        assert_eq!(err.code(), ErrorCode::NotFound, "{name:?}");
    }
}

#[test]
fn read_song_file_rejects_chart_paths_that_leave_songs() {
    let (songs, _) = song_tree();
    let inner = songs.path().join("123 Artist - Title");
    for chart in [
        Path::new("../map.osu").to_path_buf(),
        Path::new("../123 Artist - Title/map [7K].osu").to_path_buf(),
        inner.join("map [7K].osu"),
        std::path::PathBuf::new(),
    ] {
        assert_eq!(
            read_song_file(&inner, &chart, "secret.mp3", CAP).unwrap_err(),
            SongFileError::Missing,
            "{chart:?}"
        );
    }
}

#[test]
fn read_song_file_refuses_files_above_the_cap() {
    let (songs, chart) = song_tree();
    let len = b"ID3 audio".len() as u64;
    assert_eq!(
        read_song_file(songs.path(), &chart, "audio.mp3", len).unwrap(),
        b"ID3 audio",
        "a file of exactly the cap is read"
    );
    let err = read_song_file(songs.path(), &chart, "audio.mp3", len - 1).unwrap_err();
    assert_eq!(
        err,
        SongFileError::TooLarge {
            size: len,
            max: len - 1
        }
    );
    assert_eq!(err.code(), ErrorCode::UnsupportedFormat);
}

#[test]
fn read_song_file_reports_a_missing_file() {
    let (songs, chart) = song_tree();
    assert_eq!(
        read_song_file(songs.path(), &chart, "gone.mp3", CAP).unwrap_err(),
        SongFileError::Missing
    );
    assert_eq!(
        read_song_file(songs.path(), Path::new("gone/map.osu"), "audio.mp3", CAP).unwrap_err(),
        SongFileError::Missing
    );
    fs::create_dir(songs.path().join("123 Artist - Title").join("dir.mp3")).unwrap();
    assert_eq!(
        read_song_file(songs.path(), &chart, "dir.mp3", CAP).unwrap_err(),
        SongFileError::Missing,
        "a directory is not a song file"
    );
}
