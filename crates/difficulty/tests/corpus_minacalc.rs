#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! ADR 0022 acceptance on the pilot's library: v527 orders the REFORM 2nd 4K dans strictly by
//! Overall MSD at 1.0x. Read-only over `WOLLUF_CORPUS`.

use std::fs;
use std::path::{Path, PathBuf};

use wolluf_chart::{Chart, ChartDecoder, OsuDecoder};
use wolluf_difficulty::minacalc::{MinaCalcParams, MsdStatus, msd_table};
use wolluf_minacalc::Calc;

const CORPUS_ENV: &str = "WOLLUF_CORPUS";
const PACK_TITLE: &str = "Dan ~ REFORM ~ 2nd Pack";
/// Ladder order; the pilot's copy has no Alpha chart, so missing stages are skipped.
const DANS: [&str; 10] = [
    "~ 6th ~",
    "~ 7th ~",
    "~ 8th ~",
    "~ 9th ~",
    "~ 10th ~",
    "~ EXTRA-ALPHA ~",
    "~ EXTRA-BETA ~",
    "~ EXTRA-GAMMA ~",
    "~ EXTRA-DELTA ~",
    "~ EXTRA-EPSILON ~",
];
const MIN_STAGES: usize = 5;

fn header<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.strip_prefix(key))
        .map(str::trim)
}

/// Song folders are `<set id> <artist> - <title>`, so only folders naming REFORM are opened.
fn reform_charts(songs: &Path) -> Vec<(usize, PathBuf, Chart)> {
    let mut found = Vec::new();
    for dir in fs::read_dir(songs).unwrap() {
        let dir = dir.unwrap().path();
        let name = dir.file_name().unwrap().to_string_lossy().to_uppercase();
        if !dir.is_dir() || !name.contains("REFORM") {
            continue;
        }
        for file in fs::read_dir(&dir).unwrap() {
            let path = file.unwrap().path();
            if path.extension().is_none_or(|e| e != "osu") {
                continue;
            }
            let bytes = fs::read(&path).unwrap();
            let text = String::from_utf8_lossy(&bytes);
            if header(&text, "Title:") != Some(PACK_TITLE) {
                continue;
            }
            let Some(version) = header(&text, "Version:") else {
                continue;
            };
            let Some(stage) = DANS.iter().position(|d| version.starts_with(d)) else {
                continue;
            };
            let decoded = OsuDecoder.decode(&bytes).unwrap();
            found.push((stage, path, decoded.chart));
        }
    }
    found.sort_by_key(|(stage, _, _)| *stage);
    found
}

#[test]
#[ignore = "corpus: needs WOLLUF_CORPUS"]
fn corpus_minacalc_reform_order() {
    let Some(root) = std::env::var_os(CORPUS_ENV) else {
        panic!(
            "{CORPUS_ENV} is unset; run: WOLLUF_CORPUS=\"/mnt/e/Games/osu!\" cargo nextest run -p wolluf-difficulty --run-ignored only --release"
        );
    };
    let charts = reform_charts(&PathBuf::from(root).join("Songs"));
    let stages: Vec<usize> = charts.iter().map(|(s, _, _)| *s).collect();
    let mut unique = stages.clone();
    unique.dedup();
    assert_eq!(unique, stages, "a dan stage appears twice");
    assert!(
        charts.len() >= MIN_STAGES,
        "found {} REFORM 2nd stages, expected at least {MIN_STAGES}",
        charts.len()
    );

    let params = MinaCalcParams {
        rate_grid_milli: vec![1000],
        ..MinaCalcParams::default()
    };
    let mut calc = Calc::new().unwrap();
    let mut overall = Vec::new();
    for (stage, path, chart) in &charts {
        let table = msd_table(&mut calc, chart, &params);
        assert_eq!(table.status, MsdStatus::Rated, "{}", path.display());
        let centi = table.rows[0].centi;
        eprintln!(
            "{:<20} overall {:>6.2}  {centi:?}",
            DANS[*stage],
            f64::from(centi[0]) / 100.0
        );
        overall.push((DANS[*stage], centi[0]));
    }
    for w in overall.windows(2) {
        assert!(w[0].1 < w[1].1, "not strictly ordered: {overall:?}");
    }
}
