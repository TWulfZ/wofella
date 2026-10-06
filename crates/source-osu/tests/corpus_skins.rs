#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Corpus harness for `skins`: the pilot's `Skins/` lists, every 7K skin loads under the default
//! caps, and the active skin resolves notes and keys for all 7 columns. `#[ignore]`, and
//! `WOLLUF_CORPUS` is only read, never written.

use std::collections::BTreeMap;
use std::path::PathBuf;

use wolluf_source_osu::cfg_files::{list_user_cfgs, read_user_cfg};
use wolluf_source_osu::skins::{SkinsParams, list_skins, load_skin};

const CORPUS_ENV: &str = "WOLLUF_CORPUS";
const SKINS_DIR: &str = "Skins";
const MANIA_7K: u8 = 7;
/// The pilot install has 25 skins with a `Keys: 7` block (2026-10-06 survey); 20 leaves room for
/// skins being removed without letting a lister that drops blocks pass.
const MIN_7K_SKINS: usize = 20;

fn corpus_root() -> PathBuf {
    let Some(raw) = std::env::var_os(CORPUS_ENV) else {
        panic!(
            "{CORPUS_ENV} is unset; e.g. {CORPUS_ENV}=\"/mnt/e/Games/osu!\" cargo nextest run \
             -p wolluf-source-osu --run-ignored only corpus_skins"
        );
    };
    PathBuf::from(raw)
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn corpus_skins_list_load_and_active_skin() {
    let root = corpus_root();
    let skins_dir = root.join(SKINS_DIR);
    let params = SkinsParams::default();
    let list = list_skins(&skins_dir, &params).unwrap();
    let seven: Vec<&str> = list
        .iter()
        .filter(|e| e.keys.contains(&MANIA_7K))
        .map(|e| e.folder.as_str())
        .collect();
    eprintln!("{} skins listed, {} with Keys: 7", list.len(), seven.len());
    assert!(
        seven.len() >= MIN_7K_SKINS,
        "only {} skins have Keys: 7",
        seven.len()
    );

    let mut largest = 0;
    let mut codes = BTreeMap::new();
    for folder in &seven {
        let skin = load_skin(&skins_dir, folder, MANIA_7K, &params)
            .unwrap_or_else(|e| panic!("{folder:?}: {e}"));
        let bytes: usize = skin.files.iter().map(|f| f.bytes.len()).sum();
        largest = largest.max(bytes);
        for d in &skin.diagnostics {
            *codes.entry(d.code.as_str()).or_insert(0) += 1;
        }
    }
    eprintln!("largest 7K working set: {largest} bytes; diagnostics {codes:?}");

    // Only `skin` is used: the cfg also holds a Password line that must never be read or printed
    // (spec 002 R9), and the codec skips it before touching its value.
    let cfgs = list_user_cfgs(&root).unwrap();
    let newest = cfgs.first().expect("no osu!.<account>.cfg in the corpus");
    let folder = read_user_cfg(&newest.path)
        .unwrap()
        .0
        .skin
        .expect("the pilot cfg sets Skin");
    let skin = load_skin(&skins_dir, &folder, MANIA_7K, &params).unwrap();
    let bytes: usize = skin.files.iter().map(|f| f.bytes.len()).sum();
    eprintln!(
        "active skin: {} slots, {} files, {bytes} bytes, diagnostics {:?}",
        skin.images.len(),
        skin.files.len(),
        skin.diagnostics
            .iter()
            .map(|d| format!("{}@{}", d.code, d.slot.as_deref().unwrap_or("-")))
            .collect::<Vec<_>>()
    );
    for column in 0..usize::from(MANIA_7K) {
        for slot in [format!("note.{column}"), format!("key.{column}")] {
            assert!(
                skin.images.iter().any(|i| i.slot == slot),
                "active skin does not resolve {slot}"
            );
        }
    }
}
