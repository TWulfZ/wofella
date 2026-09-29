//! `chart_parse`: `.osu` bytes to a normalized chart plus the summary columns of
//! `chart_parsed` (architecture §5.4). The key is stage-level; charts are memoized per md5 in
//! `derivation.input_key`, so no per-chart input enters it.

use wolluf_chart::{Chart, ChartDecoder, Diagnostics, OsuDecoder};
use wolluf_core::{Keymode, StageId, TimeUs, VersionKey, VersionKeyBuilder};

use crate::error::EngineError;
use crate::rows_blob::ROWS_FORMAT_V1;

pub const STAGE: StageId = StageId::from_static("chart_parse");
pub const VERSION: u32 = 1;

const CONFIG_TAG: &[u8] = b"wolluf.chart_parse.config.v1";
const US_PER_SECOND: f64 = 1_000_000.0;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedChart {
    pub chart: Chart,
    pub diagnostics: Diagnostics,
    pub summary: ChartSummary,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChartSummary {
    pub keymode: Keymode,
    /// Taps plus LN heads: an LN counts once.
    pub n_notes: u32,
    pub n_ln: u32,
    /// `n_ln / n_notes`, and 0.0 without notes: the store column is `REAL NOT NULL` and SQLite
    /// turns NaN into NULL.
    pub ln_ratio: f64,
    /// First object to last object, LN tails included, floored to whole ms.
    pub length_ms: u32,
    /// `n_notes` over `length`; 0.0 when every object sits at one instant.
    pub nps: f64,
}

pub fn parse_chart(bytes: &[u8]) -> Result<ParsedChart, EngineError> {
    let decoded = OsuDecoder.decode(bytes)?;
    let summary = summarize(&decoded.chart);
    Ok(ParsedChart {
        chart: decoded.chart,
        diagnostics: decoded.diagnostics,
        summary,
    })
}

pub fn summarize(chart: &Chart) -> ChartSummary {
    let rows = chart.rows();
    let n_taps: u32 = rows.iter().map(|r| r.tap.len()).sum();
    let n_ln: u32 = rows.iter().map(|r| r.ln_head.len()).sum();
    let n_notes = n_taps + n_ln;
    let length_us = match (rows.first(), rows.last()) {
        (Some(first), Some(last)) => last.t.0.saturating_sub(first.t.0),
        _ => 0,
    };
    let ratio = |num: f64, den: f64| if den > 0.0 { num / den } else { 0.0 };
    ChartSummary {
        keymode: chart.keymode(),
        n_notes,
        n_ln,
        ln_ratio: ratio(f64::from(n_ln), f64::from(n_notes)),
        length_ms: u32::try_from(TimeUs(length_us).as_ms_floor()).unwrap_or(u32::MAX),
        nps: ratio(f64::from(n_notes), length_us as f64 / US_PER_SECOND),
    }
}

/// The stage-level key of `chart_parsed` rows.
pub fn vkey() -> Result<VersionKey, EngineError> {
    Ok(VersionKeyBuilder::new(STAGE, VERSION)
        .config(config_hash())
        .finish()?)
}

/// What changes stored output besides `VERSION`: the decoder's format and the rows-blob
/// format, which the stage-lock golden does not see because it hashes the decoded chart.
fn config_hash() -> [u8; 32] {
    let decoder = OsuDecoder.format_id().as_bytes();
    let mut hasher = blake3::Hasher::new();
    hasher.update(CONFIG_TAG);
    hasher.update(&(decoder.len() as u32).to_le_bytes());
    hasher.update(decoder);
    hasher.update(&[ROWS_FORMAT_V1]);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use wolluf_chart::testkit::OsuText;
    use wolluf_chart::{ChartError, DiagCode, chart};
    use wolluf_core::{Keymode, TimeUs};

    use super::*;

    fn bytes_of(chart: &Chart) -> Vec<u8> {
        OsuText::from_chart(chart).build().into_bytes()
    }

    #[test]
    fn stage_id_and_version_are_stable() {
        assert_eq!(STAGE.as_str(), "chart_parse");
        assert_eq!(VERSION, 1);
    }

    #[test]
    fn summary_counts_taps_and_ln_heads_as_notes() {
        let chart = chart![step = 250;
            "x..[...",
            "x..|..x",
            "...]...",
            "[x.....",
            "]......",
        ];
        let parsed = parse_chart(&bytes_of(&chart)).unwrap();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.chart.rows(), chart.rows());
        assert_eq!(
            parsed.summary,
            ChartSummary {
                keymode: Keymode::K7,
                n_notes: 6,
                n_ln: 2,
                ln_ratio: 2.0 / 6.0,
                length_ms: 1000,
                nps: 6.0,
            }
        );
    }

    #[test]
    fn zero_note_chart_has_zero_ratio_length_and_nps() {
        let parsed = parse_chart(OsuText::mania(7).build().as_bytes()).unwrap();
        let s = parsed.summary;
        assert_eq!((s.n_notes, s.n_ln, s.length_ms), (0, 0, 0));
        assert!(!s.ln_ratio.is_nan() && !s.nps.is_nan());
        assert_eq!((s.ln_ratio, s.nps), (0.0, 0.0));
    }

    #[test]
    fn ln_only_chart_length_ends_at_the_last_tail() {
        let chart = chart![step = 100, start = 5000;
            "[.[....",
            "|.|....",
            "].|....",
            "..]....",
        ];
        let s = parse_chart(&bytes_of(&chart)).unwrap().summary;
        assert_eq!((s.n_notes, s.n_ln, s.length_ms), (2, 2, 300));
        assert_eq!(s.ln_ratio, 1.0);
        assert!((s.nps - 2.0 / 0.3).abs() < 1e-9, "{}", s.nps);
    }

    #[test]
    fn a_single_instant_has_no_rate() {
        let chart = chart![step = 100; "x.x.x.x"];
        let s = parse_chart(&bytes_of(&chart)).unwrap().summary;
        assert_eq!((s.n_notes, s.length_ms, s.nps), (4, 0, 0.0));
    }

    #[test]
    fn sub_millisecond_length_floors() {
        let text = OsuText::mania(7)
            .tap(0, TimeUs(0))
            .tap(1, TimeUs(1_999))
            .build();
        assert_eq!(parse_chart(text.as_bytes()).unwrap().summary.length_ms, 1);
    }

    #[test]
    fn keymode_follows_the_chart() {
        let chart = chart![step = 100; "x..x", ".xx."];
        let s = parse_chart(&bytes_of(&chart)).unwrap().summary;
        assert_eq!((s.keymode, s.n_notes), (Keymode::K4, 4));
    }

    #[test]
    fn decoder_diagnostics_are_kept() {
        let text = OsuText::mania(7)
            .tap(1, TimeUs::from_ms(100))
            .tap(1, TimeUs::from_ms(100))
            .build();
        let parsed = parse_chart(text.as_bytes()).unwrap();
        assert_eq!(parsed.diagnostics.codes(), [DiagCode::DuplicateNote]);
        assert_eq!(parsed.summary.n_notes, 1);
    }

    #[test]
    fn non_mania_charts_are_rejected() {
        let text = OsuText::mania(7).general("Mode", "0").build();
        assert!(matches!(
            parse_chart(text.as_bytes()),
            Err(EngineError::Chart(ChartError::UnsupportedMode(0)))
        ));
    }

    // Frozen: a change re-keys every stored chart_parsed row, so it needs a reason (a VERSION
    // bump or a rows-blob format change), never a silent edit.
    #[test]
    fn vkey_is_stage_level_and_frozen() {
        let key = vkey().unwrap();
        assert_eq!(key, vkey().unwrap());
        assert_eq!(
            key.to_string(),
            "15039789358db09bc19c58cc2008662edb9cd5b0b78f1063afcdd6c4eed3bc1a"
        );
    }
}
