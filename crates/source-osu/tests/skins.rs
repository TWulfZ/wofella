#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! `Skins/` listing and image resolution over synthetic skins: every image is a header-only PNG
//! or JPEG built here, never a real skin file.

use std::fs;
use std::path::{Path, PathBuf};

use wolluf_core::ErrorCode;
use wolluf_source_osu::codec::skin_ini::NoteBodyStyle;
use wolluf_source_osu::skins::{
    ImageKind, LoadedSkin, SkinDiagCode, SkinError, SkinFile, SkinsParams, list_skins, load_skin,
};

const FOLDER: &str = "-   #마지막 스킨 (Test, v2)";
const SEVEN_K: &str = "[General]\r\nName: Test\r\nVersion: 2.5\r\n[Mania]\r\nKeys: 7\r\n";

/// Signature plus an IHDR chunk: enough for a header check, not a decodable image.
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    out.extend_from_slice(&13u32.to_be_bytes());
    out.extend_from_slice(b"IHDR");
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&[8, 6, 0, 0, 0]);
    out.extend_from_slice(&[0; 4]);
    out
}

/// SOI, an APP0 segment to skip, then a frame header of type `sof`.
fn jpeg(width: u16, height: u16, sof: u8) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
    out.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
    out.extend_from_slice(&[0xFF, 0xFF, sof, 0x00, 0x0B, 8]);
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&width.to_be_bytes());
    out.extend_from_slice(&[1, 1, 0x11, 0]);
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

fn tiff() -> Vec<u8> {
    let mut out = b"II*\0".to_vec();
    out.extend_from_slice(&[0; 32]);
    out
}

fn put(dir: &Path, rel: &str, bytes: &[u8]) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// `Skins/` with one skin folder named `FOLDER` holding `skin.ini` = `ini`.
fn skin(ini: &str) -> (tempfile::TempDir, PathBuf) {
    let skins = tempfile::tempdir().unwrap();
    let dir = skins.path().join(FOLDER);
    fs::create_dir(&dir).unwrap();
    put(&dir, "skin.ini", ini.as_bytes());
    (skins, dir)
}

fn load(skins: &tempfile::TempDir) -> LoadedSkin {
    load_skin(skins.path(), FOLDER, 7, &SkinsParams::default()).unwrap()
}

fn file<'a>(skin: &'a LoadedSkin, slot: &str) -> Option<&'a SkinFile> {
    skin.images
        .iter()
        .find(|i| i.slot == slot)
        .map(|i| &skin.files[i.file])
}

fn path_of(skin: &LoadedSkin, slot: &str) -> Option<String> {
    file(skin, slot).map(|f| f.path.clone())
}

fn has_diag(skin: &LoadedSkin, code: SkinDiagCode, slot: &str) -> bool {
    skin.diagnostics
        .iter()
        .any(|d| d.code == code && d.slot.as_deref() == Some(slot))
}

#[test]
fn skins_default_names_follow_the_7k_column_types() {
    let (skins, dir) = skin(SEVEN_K);
    for name in [
        "mania-note1",
        "mania-note2",
        "mania-noteS",
        "mania-note1H",
        "mania-note1L",
        "mania-noteSL",
        "mania-key1",
        "mania-key1D",
        "mania-key2",
        "mania-keyS",
        "mania-stage-left",
        "mania-stage-hint",
        "mania-stage-bottom",
    ] {
        put(&dir, &format!("{name}.png"), &png(2, 2));
    }
    let s = load(&skins);
    let types = ["1", "2", "1", "S", "1", "2", "1"];
    for (i, t) in types.iter().enumerate() {
        assert_eq!(
            path_of(&s, &format!("note.{i}")),
            Some(format!("mania-note{t}.png")),
            "column {i}"
        );
        assert_eq!(
            path_of(&s, &format!("key.{i}")),
            Some(format!("mania-key{t}.png")),
            "column {i}"
        );
    }
    assert_eq!(
        path_of(&s, "note.0.head").as_deref(),
        Some("mania-note1H.png")
    );
    assert_eq!(
        path_of(&s, "note.1.head").as_deref(),
        Some("mania-note2.png"),
        "a missing head falls back to the note"
    );
    assert_eq!(
        path_of(&s, "note.0.tail").as_deref(),
        Some("mania-note1H.png"),
        "a missing tail falls back to the head"
    );
    assert_eq!(
        path_of(&s, "note.3.tail").as_deref(),
        Some("mania-noteS.png")
    );
    assert_eq!(path_of(&s, "body.0").as_deref(), Some("mania-note1L.png"));
    assert_eq!(path_of(&s, "body.3").as_deref(), Some("mania-noteSL.png"));
    assert_eq!(
        path_of(&s, "body.1"),
        None,
        "the body has no further fallback"
    );
    assert_eq!(
        path_of(&s, "key.0.down").as_deref(),
        Some("mania-key1D.png")
    );
    assert_eq!(path_of(&s, "key.1.down"), None);
    assert_eq!(
        path_of(&s, "stage.left").as_deref(),
        Some("mania-stage-left.png")
    );
    assert_eq!(
        path_of(&s, "stage.hint").as_deref(),
        Some("mania-stage-hint.png")
    );
    assert_eq!(
        path_of(&s, "stage.bottom").as_deref(),
        Some("mania-stage-bottom.png")
    );
    assert_eq!(path_of(&s, "stage.right"), None);
    assert!(has_diag(&s, SkinDiagCode::ImageMissing, "stage.right"));
    assert!(has_diag(&s, SkinDiagCode::ImageMissing, "body.1"));
    assert_eq!(s.name.as_deref(), Some("Test"));
    assert_eq!(s.version, 2.5);
    assert_eq!(s.config.keys, 7);
}

#[test]
fn skins_explicit_references_replace_the_default_name() {
    let (skins, dir) = skin(
        "[General]\nVersion: 2.5\n[Mania]\nKeys: 7\nNoteImage0: Notes\\Sub\\note\n\
         NoteImage1H: gone\nNoteImage2: nothere\nKeyImage0: KEYS/Up\nStageHint: blank.PNG\n",
    );
    put(&dir, "notes/SUB/NOTE.png", &png(3, 3));
    put(&dir, "mania-note1.png", &png(2, 2));
    put(&dir, "mania-note2.png", &png(2, 2));
    put(&dir, "keys/up.Png", &png(2, 2));
    put(&dir, "Blank.png", &png(1, 1));
    let s = load(&skins);
    assert_eq!(path_of(&s, "note.0").as_deref(), Some("notes/SUB/NOTE.png"));
    assert_eq!(
        path_of(&s, "note.1.head").as_deref(),
        Some("mania-note2.png"),
        "an explicit head that does not resolve falls back to the note chain"
    );
    assert_eq!(
        path_of(&s, "note.2"),
        None,
        "an explicit reference is not replaced by mania-note1"
    );
    assert_eq!(path_of(&s, "key.0").as_deref(), Some("keys/up.Png"));
    assert_eq!(path_of(&s, "stage.hint").as_deref(), Some("Blank.png"));
}

#[test]
fn skins_prefer_2x_and_try_png_then_jpg() {
    let (skins, dir) = skin(
        "[General]\nVersion: 2.5\n[Mania]\nKeys: 7\nNoteImage0: a@2x\nNoteImage1: b.jpg\n\
         NoteImage2: c\nKeyImage0: d\n",
    );
    put(&dir, "a.png", &png(10, 10));
    put(&dir, "A@2X.png", &png(20, 20));
    put(&dir, "b.jpg", &jpeg(8, 4, 0xC0));
    put(&dir, "c.jpg", &jpeg(6, 3, 0xC2));
    put(&dir, "d.png", &png(4, 4));
    let s = load(&skins);
    let a = file(&s, "note.0").unwrap();
    assert_eq!((a.path.as_str(), a.scale), ("A@2X.png", 2));
    assert_eq!((a.width, a.height), (20, 20));
    let b = file(&s, "note.1").unwrap();
    assert_eq!((b.kind, b.width, b.height), (ImageKind::Jpeg, 8, 4));
    assert_eq!(b.kind.mime(), "image/jpeg");
    let c = file(&s, "note.2").unwrap();
    assert_eq!((c.path.as_str(), c.width, c.height), ("c.jpg", 6, 3));
    let d = file(&s, "key.0").unwrap();
    assert_eq!(
        (d.kind, d.scale, d.kind.mime()),
        (ImageKind::Png, 1, "image/png")
    );
}

#[test]
fn skins_first_frame_only_for_animated_elements() {
    let (skins, dir) = skin(SEVEN_K);
    put(&dir, "mania-note1-0.png", &png(2, 2));
    put(&dir, "mania-note1-1.png", &png(2, 2));
    put(&dir, "mania-note2.png", &png(2, 2));
    put(&dir, "mania-note2-0.png", &png(9, 9));
    put(&dir, "mania-noteS-0@2x.png", &png(4, 4));
    put(&dir, "mania-key1-0.png", &png(2, 2));
    put(&dir, "mania-stage-bottom-0.png", &png(2, 2));
    let s = load(&skins);
    assert_eq!(path_of(&s, "note.0").as_deref(), Some("mania-note1-0.png"));
    let frame = file(&s, "note.1").unwrap();
    assert_eq!(
        (frame.path.as_str(), frame.width),
        ("mania-note2-0.png", 9),
        "frame 0 comes before the plain name"
    );
    let special = file(&s, "note.3").unwrap();
    assert_eq!(
        (special.path.as_str(), special.scale),
        ("mania-noteS-0@2x.png", 2)
    );
    assert_eq!(path_of(&s, "key.0"), None, "keys are not animated in lazer");
    assert_eq!(
        path_of(&s, "stage.bottom").as_deref(),
        Some("mania-stage-bottom-0.png")
    );
}

#[test]
fn skins_frame_zero_wins_over_the_plain_name() {
    let (skins, dir) = skin(SEVEN_K);
    put(&dir, "mania-note1.png", &png(2, 2));
    put(&dir, "mania-note1-0.png", &png(3, 3));
    put(&dir, "mania-key1.png", &png(2, 2));
    put(&dir, "mania-key1-0.png", &png(3, 3));
    let s = load(&skins);
    assert_eq!(path_of(&s, "note.0").as_deref(), Some("mania-note1-0.png"));
    assert_eq!(
        path_of(&s, "key.0").as_deref(),
        Some("mania-key1.png"),
        "keys are not animated, so their frame 0 is never tried"
    );
}

#[test]
fn skins_non_png_jpeg_is_a_missing_slot() {
    let (skins, dir) =
        skin("[General]\nVersion: 2.5\n[Mania]\nKeys: 7\nNoteImage0L: 4K\\LN_Body\n");
    put(&dir, "4K/LN_Body.png", &tiff());
    put(&dir, "mania-note2L@2x.png", &tiff());
    put(&dir, "mania-note2L.png", &png(5, 50));
    put(&dir, "mania-noteS.png", &png(0, 10));
    put(&dir, "mania-note1.png", b"\x89PNG\r\n\x1a\n");
    let s = load(&skins);
    assert_eq!(path_of(&s, "body.0"), None);
    assert!(has_diag(&s, SkinDiagCode::ImageBadFormat, "body.0"));
    assert!(has_diag(&s, SkinDiagCode::ImageMissing, "body.0"));
    let body = file(&s, "body.1").unwrap();
    assert_eq!(
        (body.path.as_str(), body.scale),
        ("mania-note2L.png", 1),
        "a rejected @2x file falls through to the plain one"
    );
    assert!(has_diag(&s, SkinDiagCode::ImageBadHeader, "note.3"));
    assert!(has_diag(&s, SkinDiagCode::ImageBadHeader, "note.0"));
    assert_eq!(path_of(&s, "note.0"), None);
}

#[test]
fn skins_jpeg_frames_other_than_baseline_and_progressive_are_rejected() {
    let (skins, dir) =
        skin("[General]\nVersion: 2.5\n[Mania]\nKeys: 7\nNoteImage0: l\nNoteImage1: e\n");
    put(&dir, "l.jpg", &jpeg(4, 4, 0xC3));
    put(&dir, "e.jpg", &[0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02]);
    let s = load(&skins);
    assert!(has_diag(&s, SkinDiagCode::ImageBadHeader, "note.0"));
    assert!(has_diag(&s, SkinDiagCode::ImageBadHeader, "note.1"));
}

#[test]
fn skins_references_that_leave_the_skin_are_rejected() {
    let skins = tempfile::tempdir().unwrap();
    let dir = skins.path().join(FOLDER);
    put(skins.path(), "outside.png", &png(2, 2));
    let absolute = skins.path().join("outside").display().to_string();
    // `a\\d` stands for an empty segment: `a//d` would start a skin.ini comment.
    let ini = format!(
        "[General]\nVersion: 2.5\n[Mania]\nKeys: 7\nNoteImage0: ../outside\nNoteImage1: {absolute}\n\
         NoteImage2: C:/outside\nNoteImage3: C:outside\nNoteImage4: a/b/c/d/e\n\
         NoteImage5: a/b/c/d\nNoteImage6: a\\\\d\nKeyImage0: ./mania-key1\nKeyImage1: a/b/../../x\n"
    );
    put(&dir, "skin.ini", ini.as_bytes());
    put(&dir, "a/b/c/d/e.png", &png(2, 2));
    put(&dir, "a/b/c/d.png", &png(3, 3));
    put(&dir, "a/d.png", &png(2, 2));
    put(&dir, "mania-key1.png", &png(2, 2));
    put(&dir, "x.png", &png(2, 2));
    let s = load(&skins);
    for slot in [
        "note.0", "note.1", "note.2", "note.3", "note.4", "note.6", "key.0", "key.1",
    ] {
        assert_eq!(path_of(&s, slot), None, "{slot}");
        assert!(has_diag(&s, SkinDiagCode::RefRejected, slot), "{slot}");
    }
    assert_eq!(
        path_of(&s, "note.5").as_deref(),
        Some("a/b/c/d.png"),
        "depth 4 is allowed"
    );
}

/// ADR 0019: links are followed on purpose; only the reference depth bounds a link loop.
#[cfg(unix)]
#[test]
fn skins_links_are_followed_and_loops_stop_at_the_depth_cap() {
    use std::os::unix::fs::symlink;

    let outside = tempfile::tempdir().unwrap();
    put(outside.path(), "Shared/note.png", &png(4, 4));
    put(outside.path(), "key.png", &png(5, 5));
    let moved = outside.path().join("moved skin");
    put(
        &moved,
        "skin.ini",
        b"[General]\nVersion: 2.5\n[Mania]\nKeys: 7\nNoteImage0: linked/note\n\
          NoteImage1: loop/loop/loop/mania-note1\nNoteImage2: loop/loop/loop/loop/mania-note1\n",
    );
    put(&moved, "mania-note1.png", &png(2, 2));
    symlink(outside.path().join("Shared"), moved.join("linked")).unwrap();
    symlink(outside.path().join("key.png"), moved.join("mania-key1.png")).unwrap();
    symlink(".", moved.join("loop")).unwrap();
    let skins = tempfile::tempdir().unwrap();
    symlink(&moved, skins.path().join("Moved")).unwrap();

    let s = load_skin(skins.path(), "Moved", 7, &SkinsParams::default()).unwrap();
    assert_eq!(s.folder, "Moved");
    let note = file(&s, "note.0").unwrap();
    assert_eq!((note.path.as_str(), note.width), ("linked/note.png", 4));
    let key = file(&s, "key.0").unwrap();
    assert_eq!((key.path.as_str(), key.width), ("mania-key1.png", 5));
    assert_eq!(
        path_of(&s, "note.1").as_deref(),
        Some("loop/loop/loop/mania-note1.png"),
        "a loop is followed up to the depth cap"
    );
    assert_eq!(path_of(&s, "note.2"), None);
    assert!(has_diag(&s, SkinDiagCode::RefRejected, "note.2"));
}

#[test]
fn skins_folder_names_must_be_one_entry_of_skins() {
    let (skins, _) = skin(SEVEN_K);
    let params = SkinsParams::default();
    put(skins.path(), "loose.png", &png(1, 1));
    for name in [
        "",
        "  ",
        ".",
        "..",
        "../x",
        "a/b",
        "a\\b",
        "a\0b",
        "missing",
        "loose.png",
    ] {
        let err = load_skin(skins.path(), name, 7, &params).unwrap_err();
        assert_eq!(err, SkinError::Missing, "{name:?}");
        assert_eq!(err.code(), ErrorCode::NotFound);
    }
    let absolute = skins.path().join(FOLDER).display().to_string();
    assert_eq!(
        load_skin(skins.path(), &absolute, 7, &params).unwrap_err(),
        SkinError::Missing
    );
    assert!(load_skin(skins.path(), FOLDER, 7, &params).is_ok());
}

#[test]
fn skins_folder_must_equal_a_listed_entry_byte_for_byte() {
    let (skins, _) = skin(SEVEN_K);
    put(skins.path(), "s/skin.ini", SEVEN_K.as_bytes());
    let params = SkinsParams::default();
    // On NTFS these open `s`; the listing check must turn them away there too.
    for name in ["S", "s.", "s "] {
        assert_eq!(
            load_skin(skins.path(), name, 7, &params).unwrap_err(),
            SkinError::Missing,
            "{name:?}"
        );
    }
    assert_eq!(
        load_skin(skins.path(), "s", 7, &params).unwrap().folder,
        "s"
    );

    // A folder that opens on disk but is outside the listing (here: past the cap) is not a skin.
    assert!(skins.path().join(FOLDER).is_dir());
    let unlisted = SkinsParams {
        max_skins: 0,
        ..SkinsParams::default()
    };
    assert_eq!(
        load_skin(skins.path(), FOLDER, 7, &unlisted).unwrap_err(),
        SkinError::Missing
    );
}

#[test]
fn skins_invalid_keymode_is_invalid_input() {
    let (skins, _) = skin(SEVEN_K);
    for keys in [0, 19] {
        let err = load_skin(skins.path(), FOLDER, keys, &SkinsParams::default()).unwrap_err();
        assert_eq!(err, SkinError::InvalidKeymode { keys });
        assert_eq!(err.code(), ErrorCode::InvalidInput);
    }
}

#[test]
fn skins_byte_and_area_caps() {
    let (skins, dir) = skin(SEVEN_K);
    let mut big = png(2, 2);
    big.resize(2_000, 0);
    put(&dir, "mania-note1.png", &big);
    put(&dir, "mania-note2.png", &png(3_000, 3_000));
    put(&dir, "mania-note1L.png", &png(138, 40_000));
    let params = SkinsParams {
        max_image_bytes: 1_999,
        ..SkinsParams::default()
    };
    let s = load_skin(skins.path(), FOLDER, 7, &params).unwrap();
    assert_eq!(path_of(&s, "note.0"), None);
    assert!(has_diag(&s, SkinDiagCode::ImageTooLarge, "note.0"));
    assert_eq!(path_of(&s, "note.1"), None);
    assert!(has_diag(&s, SkinDiagCode::ImageTooManyPixels, "note.1"));
    let body = file(&s, "body.0").unwrap();
    assert_eq!(
        (body.width, body.height),
        (138, 40_000),
        "tall LN bodies pass"
    );

    let total = SkinsParams {
        max_skin_bytes: 2_010,
        ..SkinsParams::default()
    };
    let err = load_skin(skins.path(), FOLDER, 7, &total).unwrap_err();
    assert!(
        matches!(err, SkinError::TooLarge { max: 2_010, .. }),
        "{err:?}"
    );
    assert_eq!(err.code(), ErrorCode::UnsupportedFormat);
}

#[test]
fn skins_pixel_budget_turns_further_images_into_missing_slots() {
    let (skins, dir) = skin(SEVEN_K);
    put(&dir, "mania-note1.png", &png(2, 2));
    put(&dir, "mania-note2.png", &png(3, 3));
    put(&dir, "mania-noteS.png", &png(1, 1));
    assert_eq!(SkinsParams::default().max_total_pixels, 64 * 1_024 * 1_024);
    let params = SkinsParams {
        max_total_pixels: 10,
        ..SkinsParams::default()
    };
    let s = load_skin(skins.path(), FOLDER, 7, &params).unwrap();
    assert_eq!(path_of(&s, "note.0").as_deref(), Some("mania-note1.png"));
    assert_eq!(path_of(&s, "note.1"), None, "4 + 9 px is over the budget");
    assert!(has_diag(&s, SkinDiagCode::PixelBudgetExceeded, "note.1"));
    assert!(has_diag(&s, SkinDiagCode::ImageMissing, "note.1"));
    assert_eq!(
        path_of(&s, "note.3").as_deref(),
        Some("mania-noteS.png"),
        "an image that still fits is kept"
    );
    assert_eq!(s.files.len(), 2);
}

#[test]
fn skins_ini_above_the_cap_is_too_large() {
    let (skins, _) = skin(SEVEN_K);
    let params = SkinsParams {
        max_ini_bytes: 8,
        ..SkinsParams::default()
    };
    let err = load_skin(skins.path(), FOLDER, 7, &params).unwrap_err();
    assert_eq!(
        err,
        SkinError::TooLarge {
            size: SEVEN_K.len() as u64,
            max: 8
        }
    );
}

#[test]
fn skins_identical_files_are_carried_once() {
    let (skins, dir) = skin(
        "[General]\nVersion: 2.5\n[Mania]\nKeys: 7\nNoteImage0: mania-note1\nNoteImage1: MANIA-NOTE1\n\
         NoteImage2: mania-note1.png\nNoteImage3: mania-note1\n",
    );
    put(&dir, "mania-note1.png", &png(2, 2));
    let s = load(&skins);
    let index = |slot: &str| s.images.iter().find(|i| i.slot == slot).unwrap().file;
    for slot in [
        "note.1",
        "note.2",
        "note.3",
        "note.4",
        "note.6",
        "note.0.head",
        "note.0.tail",
    ] {
        assert_eq!(index(slot), index("note.0"), "{slot}");
    }
    assert_eq!(s.files.len(), 1);
    assert_eq!(s.files[0].bytes, png(2, 2));
}

#[test]
fn skins_directories_named_like_images_are_not_images() {
    let (skins, dir) = skin(SEVEN_K);
    fs::create_dir_all(dir.join("mania-note1.png")).unwrap();
    fs::create_dir_all(dir.join("mania-note2")).unwrap();
    put(&dir, "mania-note2.png", &png(2, 2));
    let s = load(&skins);
    assert_eq!(path_of(&s, "note.0"), None);
    assert_eq!(path_of(&s, "note.1").as_deref(), Some("mania-note2.png"));
}

#[test]
fn skins_without_ini_or_keys_block_use_lazer_defaults() {
    let skins = tempfile::tempdir().unwrap();
    let dir = skins.path().join(FOLDER);
    put(&dir, "mania-note1.png", &png(2, 2));
    let s = load(&skins);
    assert_eq!(s.name, None);
    assert_eq!(
        s.version, 2.7,
        "lazer treats a skin without skin.ini as latest"
    );
    assert_eq!(s.config.column_width, vec![30.0; 7]);
    assert_eq!(s.config.note_body_style, NoteBodyStyle::RepeatBottom);
    assert!(
        s.diagnostics
            .iter()
            .any(|d| d.code == SkinDiagCode::IniMissing)
    );
    assert_eq!(path_of(&s, "note.0").as_deref(), Some("mania-note1.png"));

    put(
        &dir,
        "Skin.INI",
        b"[General]\nName: Four\n[Mania]\nKeys: 4\nHitPosition: 300\n",
    );
    let s = load(&skins);
    assert_eq!(s.name.as_deref(), Some("Four"));
    assert_eq!(s.version, 1.0);
    assert_eq!(s.config.hit_position, 402.0);
    assert_eq!(s.config.note_body_style, NoteBodyStyle::Stretch);
    assert!(
        s.diagnostics
            .iter()
            .any(|d| d.code == SkinDiagCode::KeysBlockMissing)
    );
    assert!(
        !s.diagnostics
            .iter()
            .any(|d| d.code == SkinDiagCode::IniMissing)
    );
}

#[test]
fn skins_ini_is_found_past_a_truncated_listing() {
    let (skins, dir) = skin(SEVEN_K);
    for i in 0..8 {
        put(&dir, &format!("junk{i}.txt"), b"");
    }
    let params = SkinsParams {
        max_dir_entries: 0,
        ..SkinsParams::default()
    };
    for spelling in ["skin.ini", "Skin.ini"] {
        if spelling != "skin.ini" {
            fs::rename(dir.join("skin.ini"), dir.join(spelling)).unwrap();
        }
        let s = load_skin(skins.path(), FOLDER, 7, &params).unwrap();
        assert_eq!(s.name.as_deref(), Some("Test"), "{spelling}");
        assert_eq!(s.version, 2.5, "{spelling}");
        let codes: Vec<SkinDiagCode> = s.diagnostics.iter().map(|d| d.code).collect();
        assert!(
            codes.contains(&SkinDiagCode::ListingTruncated),
            "{spelling}"
        );
        assert!(!codes.contains(&SkinDiagCode::IniMissing), "{spelling}");

        let listed = list_skins(skins.path(), &params).unwrap();
        assert!(listed[0].has_ini, "{spelling}");
        assert_eq!(listed[0].keys, [7], "{spelling}");
    }
}

#[test]
fn skins_list_reads_names_and_key_counts() {
    let skins = tempfile::tempdir().unwrap();
    put(
        skins.path(),
        "R Skin v3.0/skin.ini",
        b"[General]\nName: R\n[Mania]\nKeys: 7\n[Mania]\nKeys: 4\n",
    );
    put(
        skins.path(),
        &format!("{FOLDER}/Skin.ini"),
        b"[General]\nName: # Mono\n",
    );
    fs::create_dir(skins.path().join("User")).unwrap();
    put(skins.path(), "readme.txt", b"not a skin");
    fs::create_dir_all(skins.path().join("Dir/skin.ini")).unwrap();
    let list = list_skins(skins.path(), &SkinsParams::default()).unwrap();
    let folders: Vec<&str> = list.iter().map(|e| e.folder.as_str()).collect();
    assert_eq!(folders, [FOLDER, "Dir", "R Skin v3.0", "User"]);
    assert_eq!(list[0].name.as_deref(), Some("# Mono"));
    assert!(list[0].has_ini);
    assert!(list[0].keys.is_empty());
    assert!(!list[1].has_ini, "a directory named skin.ini is not an ini");
    assert!(
        list[0].ini_mtime.is_some(),
        "the UI keys its skin cache on it (ADR 0019)"
    );
    assert_eq!(list[1].ini_mtime, None);
    assert_eq!(list[2].keys, [4, 7]);
    assert_eq!(list[2].name.as_deref(), Some("R"));
    assert!(!list[3].has_ini);
    assert_eq!(list[3].name, None);

    let capped = SkinsParams {
        max_skins: 2,
        ..SkinsParams::default()
    };
    assert!(list_skins(skins.path(), &capped).unwrap().len() <= 2);

    let err = list_skins(&skins.path().join("missing"), &SkinsParams::default()).unwrap_err();
    assert_eq!(err, SkinError::Missing);
}

#[test]
fn skins_hit_bursts_follow_explicit_then_default_names_frame_zero_first() {
    let (skins, dir) = skin(&format!(
        "{SEVEN_K}Hit300g: judge\\max\r\nHit50: missing-50\r\n"
    ));
    put(&dir, "judge/max-0.png", &png(10, 10));
    put(&dir, "judge/max-1.png", &png(11, 11));
    put(&dir, "judge/max.png", &png(12, 12));
    put(&dir, "MANIA-HIT300.png", &png(20, 20));
    put(&dir, "mania-hit200@2x.png", &png(30, 30));
    put(&dir, "mania-hit50.png", &png(40, 40));
    let s = load(&skins);
    assert_eq!(path_of(&s, "hit.300g").as_deref(), Some("judge/max-0.png"));
    assert_eq!(path_of(&s, "hit.300").as_deref(), Some("MANIA-HIT300.png"));
    assert_eq!(file(&s, "hit.200").map(|f| f.scale), Some(2));
    assert_eq!(
        path_of(&s, "hit.50").as_deref(),
        Some("mania-hit50.png"),
        "a broken explicit reference falls back to the default name in the skin, as the pilot's \
         active skin needs (its `mania/stage/` paths do not exist)"
    );
    assert_eq!(path_of(&s, "hit.100"), None);
    assert!(has_diag(&s, SkinDiagCode::ImageMissing, "hit.100"));
    assert_eq!(path_of(&s, "hit.0"), None);
}

#[test]
fn skins_combo_digits_use_the_fonts_prefix_or_score() {
    let (skins, dir) = skin(&format!(
        "{SEVEN_K}[Fonts]\r\nComboPrefix: Fonts\\combo\r\nComboOverlap: 4\r\n"
    ));
    for d in 0..10 {
        put(&dir, &format!("fonts/Combo-{d}.png"), &png(8, 12));
        put(&dir, &format!("score-{d}.png"), &png(9, 13));
    }
    put(&dir, "fonts/combo-5@2x.png", &png(16, 24));
    let s = load(&skins);
    for d in 0..10 {
        let want = if d == 5 {
            "fonts/combo-5@2x.png".to_owned()
        } else {
            format!("fonts/Combo-{d}.png")
        };
        assert_eq!(path_of(&s, &format!("combo.{d}")), Some(want), "{d}");
    }
    assert_eq!(s.fonts.combo_overlap, 4.0);

    let (skins, dir) = skin(SEVEN_K);
    put(&dir, "score-0.png", &png(9, 13));
    put(&dir, "score-0-0.png", &png(1, 1));
    let s = load(&skins);
    assert_eq!(
        path_of(&s, "combo.0").as_deref(),
        Some("score-0.png"),
        "lazer's default prefix; font glyphs are not animated"
    );
    assert_eq!(s.fonts.combo_overlap, 0.0);
    assert!(has_diag(&s, SkinDiagCode::ImageMissing, "combo.1"));
}

#[test]
fn skins_lighting_frames_run_until_the_first_gap() {
    let (skins, dir) = skin(&format!("{SEVEN_K}LightingL: fx\\hold\r\n"));
    for f in [0, 1, 2, 4] {
        put(&dir, &format!("LightingN-{f}.png"), &png(50, 50));
    }
    put(&dir, "lightingN.png", &png(1, 1));
    put(&dir, "fx/hold.png", &png(60, 60));
    let s = load(&skins);
    let frames = |kind: &str| -> Vec<String> {
        (0..8)
            .map_while(|f| path_of(&s, &format!("lighting.{kind}.{f}")))
            .collect()
    };
    assert_eq!(
        frames("n"),
        ["LightingN-0.png", "LightingN-1.png", "LightingN-2.png"]
    );
    assert_eq!(path_of(&s, "lighting.n.3"), None);
    assert_eq!(path_of(&s, "lighting.n.4"), None, "frames stop at the gap");
    assert_eq!(frames("l"), ["fx/hold.png"], "a still image is frame 0");

    let params = SkinsParams {
        max_effect_frames: 2,
        ..SkinsParams::default()
    };
    let s = load_skin(skins.path(), FOLDER, 7, &params).unwrap();
    assert!(path_of(&s, "lighting.n.1").is_some());
    assert_eq!(path_of(&s, "lighting.n.2"), None, "capped");

    let (skins, _) = skin(SEVEN_K);
    let s = load(&skins);
    assert!(has_diag(&s, SkinDiagCode::ImageMissing, "lighting.n.0"));
    assert!(has_diag(&s, SkinDiagCode::ImageMissing, "lighting.l.0"));
}

#[test]
fn skins_effects_over_the_byte_budget_are_dropped_not_fatal() {
    let (skins, dir) = skin(SEVEN_K);
    put(&dir, "mania-note1.png", &png(2, 2));
    put(&dir, "mania-hit300g.png", &png(4, 4));
    let core = png(2, 2).len() as u64;
    let params = SkinsParams {
        max_skin_bytes: core,
        ..SkinsParams::default()
    };
    let s = load_skin(skins.path(), FOLDER, 7, &params).unwrap();
    assert!(path_of(&s, "note.0").is_some());
    assert_eq!(path_of(&s, "hit.300g"), None);
    assert!(has_diag(&s, SkinDiagCode::EffectBudgetExceeded, "hit.300g"));
    assert_eq!(s.files.len(), 1);

    let params = SkinsParams {
        max_skin_bytes: core - 1,
        ..SkinsParams::default()
    };
    assert!(
        matches!(
            load_skin(skins.path(), FOLDER, 7, &params),
            Err(SkinError::TooLarge { .. })
        ),
        "the playfield's own slots still fail the load"
    );
}
