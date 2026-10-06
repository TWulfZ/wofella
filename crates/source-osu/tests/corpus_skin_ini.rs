#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Corpus harness for the `skin.ini` codec: every `Skins/*/skin.ini` of a real install parses
//! without panicking. `#[ignore]`, and `WOLLUF_CORPUS` is only read, never written.

use std::fs;
use std::panic::catch_unwind;
use std::path::PathBuf;

use wolluf_source_osu::codec::skin_ini::parse_skin_ini;

const CORPUS_ENV: &str = "WOLLUF_CORPUS";
const SKINS_DIR: &str = "Skins";
const SKIN_INI: &str = "skin.ini";
const MANIA_7K: u8 = 7;
/// The pilot install has 25 skins with a `Keys: 7` block (2026-10-06 survey); 20 leaves room for
/// skins being removed without letting a parser that drops blocks pass.
const MIN_7K_SKINS: usize = 20;

fn skins_root() -> PathBuf {
    let Some(raw) = std::env::var_os(CORPUS_ENV) else {
        panic!(
            "{CORPUS_ENV} is unset; e.g. {CORPUS_ENV}=\"/mnt/e/Games/osu!\" cargo nextest run \
             -p wolluf-source-osu --run-ignored only corpus_skin_ini"
        );
    };
    let skins = PathBuf::from(raw).join(SKINS_DIR);
    assert!(skins.is_dir(), "{} is not a directory", skins.display());
    skins
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn corpus_skin_ini_every_skin_parses() {
    let mut inis: Vec<PathBuf> = fs::read_dir(skins_root())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .filter_map(|dir| {
            fs::read_dir(&dir)
                .ok()?
                .filter_map(Result::ok)
                // stable names it either way; the pilot has both `skin.ini` and `Skin.ini`.
                .find(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(SKIN_INI)
                })
                .map(|e| e.path())
                .filter(|p| p.is_file())
        })
        .collect();
    inis.sort();
    assert!(!inis.is_empty(), "no skin.ini under {SKINS_DIR}");

    let mut panicked = Vec::new();
    let mut with_7k = 0;
    for path in &inis {
        let bytes = fs::read(path).unwrap();
        match catch_unwind(|| parse_skin_ini(&bytes)) {
            Ok(ini) if ini.mania(MANIA_7K).is_some() => with_7k += 1,
            Ok(_) => {}
            Err(_) => panicked.push(path.display().to_string()),
        }
    }
    eprintln!("{} skin.ini parsed, {with_7k} with Keys: 7", inis.len());
    assert!(panicked.is_empty(), "parser panicked on {panicked:?}");
    assert!(
        with_7k >= MIN_7K_SKINS,
        "only {with_7k} skins have a Keys: 7 block"
    );
}
