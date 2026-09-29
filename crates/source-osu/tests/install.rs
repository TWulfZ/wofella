#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use wolluf_core::ErrorCode;
use wolluf_source_osu::SourceError;
use wolluf_source_osu::codec::replay_name::ReplayFileName;
use wolluf_source_osu::install::{CandidateSource, DetectEnv, Platform, detect, validate_install};
use wolluf_source_osu::testkit::{FakeInstall, OsuDbBuilder};

fn write_tree(root: &Path, files: &BTreeMap<PathBuf, Vec<u8>>) {
    for (rel, bytes) in files {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
}

fn linux_env() -> DetectEnv {
    DetectEnv {
        platform: Platform::Linux,
        ..DetectEnv::default()
    }
}

fn missing(err: SourceError) -> Vec<&'static str> {
    match err {
        SourceError::InvalidInstall { missing, .. } => missing,
        other => panic!("expected InvalidInstall, got {other:?}"),
    }
}

#[test]
fn validate_requires_db_and_exe() {
    let dir = tempfile::tempdir().unwrap();
    let no_exe = dir.path().join("no_exe");
    write_tree(&no_exe, &FakeInstall::new().without("osu!.exe").files());
    let err = validate_install(&no_exe, &linux_env()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::OsuDirNotFound);
    assert_eq!(missing(err), vec!["osu!.exe"]);
    let no_db = dir.path().join("no_db");
    write_tree(&no_db, &FakeInstall::new().without("osu!.db").files());
    assert_eq!(
        missing(validate_install(&no_db, &linux_env()).unwrap_err()),
        vec!["osu!.db"]
    );
    let neither = dir.path().join("neither");
    write_tree(
        &neither,
        &FakeInstall::empty()
            .file("readme.txt", b"x".to_vec())
            .files(),
    );
    assert_eq!(
        missing(validate_install(&neither, &linux_env()).unwrap_err()),
        vec!["osu!.db", "osu!.exe"]
    );
    let gone = dir.path().join("gone");
    assert_eq!(
        missing(validate_install(&gone, &linux_env()).unwrap_err()),
        vec!["directory"]
    );
    // osu! runs on case-insensitive file systems, so marker names match in any case.
    let upper = dir.path().join("upper");
    let files = FakeInstall::new().files();
    let renamed = files
        .into_iter()
        .map(|(p, b)| (PathBuf::from(p.to_string_lossy().to_uppercase()), b))
        .collect();
    write_tree(&upper, &renamed);
    assert!(validate_install(&upper, &linux_env()).is_ok());
}

#[test]
fn file_path_normalizes_to_parent() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path(), &FakeInstall::new().files());
    let info = validate_install(&dir.path().join("osu!.exe"), &linux_env()).unwrap();
    assert_eq!(info.root, dir.path());
}

#[test]
fn lazer_dir_is_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path(), &FakeInstall::lazer().files());
    let err = validate_install(dir.path(), &linux_env()).unwrap_err();
    assert!(matches!(err, SourceError::LazerInstall { .. }), "{err:?}");
    assert_eq!(err.code(), ErrorCode::UnsupportedFormat);
}

#[test]
fn missing_scores_db_is_valid() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path(), &FakeInstall::new().without("scores.db").files());
    let info = validate_install(dir.path(), &linux_env()).unwrap();
    assert!(!info.has_scores_db);
    assert!(!info.has_collection_db);
    assert!(!info.has_data_r);
    assert_eq!(info.user_cfgs.len(), 1);
    assert_eq!(info.songs_dir, dir.path().join("Songs"));
}

#[test]
fn reads_osu_db_version_le_i32() {
    let dir = tempfile::tempdir().unwrap();
    let md5 = "0123456789abcdef0123456789abcdef";
    let name = ReplayFileName::parse(&format!("{md5}-134279471004225018.osr")).unwrap();
    let tree = FakeInstall::new()
        .osu_db(OsuDbBuilder::new().version(20_260_924).encode())
        .collection_db(vec![0; 8])
        .replay(&name, vec![1])
        .cfg("fixture", b"BeatmapDirectory = Maps\\7k\r\n".to_vec());
    write_tree(dir.path(), &tree.files());
    let info = validate_install(dir.path(), &linux_env()).unwrap();
    assert_eq!(info.osu_db_version, 20_260_924);
    assert!(info.has_scores_db && info.has_collection_db && info.has_data_r);
    assert_eq!(info.songs_dir, dir.path().join("Maps").join("7k"));
    // A header shorter than 4 bytes cannot carry a version.
    let short = dir.path().join("short");
    write_tree(&short, &FakeInstall::new().osu_db(vec![1, 2]).files());
    let err = validate_install(&short, &linux_env()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::ParseFailed);
}

#[test]
fn validate_translates_windows_path_on_wsl() {
    let mount = tempfile::tempdir().unwrap();
    let root = mount.path().join("e").join("Games").join("osu!");
    write_tree(&root, &FakeInstall::new().files());
    let env = DetectEnv {
        platform: Platform::Wsl,
        wsl_mount_root: mount.path().to_path_buf(),
        ..DetectEnv::default()
    };
    let info = validate_install(Path::new(r"E:\Games\osu!"), &env).unwrap();
    assert_eq!(info.root, root);
    let exe = validate_install(Path::new(r"E:\Games\osu!\osu!.exe"), &env).unwrap();
    assert_eq!(exe.root, root);
}

#[test]
fn detect_returns_invalid_with_reason() {
    let dir = tempfile::tempdir().unwrap();
    let broken = dir.path().join("broken");
    write_tree(&broken, &FakeInstall::new().without("osu!.exe").files());
    let drive = dir.path().join("drive");
    write_tree(
        &drive.join("Games").join("osu!"),
        &FakeInstall::new().files(),
    );
    let env = DetectEnv {
        osu_dir_override: Some(broken.to_string_lossy().into_owned()),
        drive_roots: vec![drive.clone()],
        ..linux_env()
    };
    let found = detect(&env);
    // `<drive>/osu!` does not exist and is a guess, so it is dropped; the env one is kept.
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].0.source, CandidateSource::Env);
    assert_eq!(missing(found[0].1.clone().unwrap_err()), vec!["osu!.exe"]);
    assert_eq!(found[1].0.source, CandidateSource::DriveScan);
    assert_eq!(
        found[1].1.as_ref().unwrap().root,
        drive.join("Games").join("osu!")
    );
    assert!(detect(&linux_env()).is_empty());
}
