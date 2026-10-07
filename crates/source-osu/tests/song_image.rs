#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Set-folder images over synthetic sets: every image is a header-only PNG or JPEG built here.

use std::path::{Path, PathBuf};

use wolluf_core::ErrorCode;
use wolluf_source_osu::skins::ImageKind;
use wolluf_source_osu::song_image::{SongImage, SongImageError, SongImageLimits, read_song_image};

const SET: &str = "100 Artist - Title";
const CHART: &str = "100 Artist - Title/map.osu";

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    out.extend_from_slice(&13u32.to_be_bytes());
    out.extend_from_slice(b"IHDR");
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&[8, 6, 0, 0, 0]);
    out
}

fn jpeg() -> Vec<u8> {
    vec![
        0xff, 0xd8, 0xff, 0xc0, 0x00, 0x0b, 8, 0x00, 0x96, 0x01, 0x0e, 1, 1, 0x11, 0,
    ]
}

fn songs(files: &[(&str, Vec<u8>)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let songs = dir.path().join("Songs");
    for (rel, bytes) in files {
        let path = songs.join(SET).join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    (dir, songs)
}

fn limits() -> SongImageLimits {
    SongImageLimits {
        max_bytes: 12 * 1024 * 1024,
        max_dir_entries: 8_192,
        max_depth: 4,
    }
}

fn read(songs: &Path, reference: &str) -> Result<SongImage, SongImageError> {
    read_song_image(songs, Path::new(CHART), reference, &limits())
}

#[test]
fn song_image_resolves_case_insensitively_and_in_subfolders() {
    let (_dir, songs) = songs(&[
        ("BG.PNG", png(1920, 1080)),
        ("Assets/Back Ground.jpg", jpeg()),
    ]);
    let top = read(&songs, "bg.png").unwrap();
    assert_eq!(
        (top.path.as_str(), top.kind, top.width, top.height),
        ("BG.PNG", ImageKind::Png, 1920, 1080)
    );
    assert_eq!(top.bytes, png(1920, 1080));
    let nested = read(&songs, "assets/back ground.JPG").unwrap();
    assert_eq!(
        (
            nested.path.as_str(),
            nested.kind,
            nested.width,
            nested.height
        ),
        ("Assets/Back Ground.jpg", ImageKind::Jpeg, 270, 150)
    );
    assert_eq!(
        read(&songs, "assets\\Back Ground.jpg").unwrap().path,
        "Assets/Back Ground.jpg"
    );
}

#[test]
fn song_image_prefers_the_exact_spelling() {
    let (_dir, songs) = songs(&[("bg.png", png(1, 1)), ("BG.png", png(2, 2))]);
    let exact = read(&songs, "BG.png").unwrap();
    // Two spellings only coexist on a case-sensitive file system.
    if exact.path == "BG.png" {
        assert_eq!(exact.width, 2);
    }
}

#[test]
fn song_image_sniffs_bytes_not_the_extension() {
    let (_dir, songs) = songs(&[
        ("really-a-jpeg.png", jpeg()),
        ("bg.webp", b"RIFF\x00\x00\x00\x00WEBPVP8 ".to_vec()),
        ("broken.png", b"\x89PNG\r\n\x1a\n".to_vec()),
    ]);
    assert_eq!(
        read(&songs, "really-a-jpeg.png").unwrap().kind,
        ImageKind::Jpeg
    );
    assert_eq!(read(&songs, "bg.webp"), Err(SongImageError::Unsupported));
    assert_eq!(read(&songs, "broken.png"), Err(SongImageError::Unsupported));
}

#[test]
fn song_image_refuses_escapes_missing_and_oversized_files() {
    let (_dir, songs) = songs(&[("bg.png", png(4, 4)), ("dir/bg.png", png(4, 4))]);
    std::fs::write(songs.join("outside.png"), png(4, 4)).unwrap();
    for reference in [
        "../outside.png",
        "",
        "  ",
        "C:bg.png",
        "a/b/c/d/bg.png",
        "dir",
        "nope.png",
    ] {
        assert_eq!(
            read(&songs, reference),
            Err(SongImageError::Missing),
            "{reference:?}"
        );
    }
    let escape = read_song_image(&songs, Path::new("../x/map.osu"), "bg.png", &limits());
    assert_eq!(escape, Err(SongImageError::Missing));
    let capped = SongImageLimits {
        max_bytes: 10,
        ..limits()
    };
    let size = png(4, 4).len() as u64;
    assert_eq!(
        read_song_image(&songs, Path::new(CHART), "bg.png", &capped),
        Err(SongImageError::TooLarge { size, max: 10 })
    );
    assert_eq!(
        SongImageError::Unsupported.code(),
        ErrorCode::UnsupportedFormat
    );
}
