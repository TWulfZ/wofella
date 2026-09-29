//! Synthetic charts for tests: the `chart!` DSL and a `.osu` text builder. Test support only;
//! nothing here is compiled into release builds.

use thiserror::Error;
use wolluf_core::{ColMask, Keymode, TimeUs};

use crate::diag::Diagnostics;
use crate::model::{Chart, ChartMeta, Note, NoteKind, TimingKind};

/// Draws a chart top-down, earliest row first: one string per row, one char per column.
/// `.` empty, `x` tap, `[` LN head, `|` LN body, `]` LN tail. Row `i` sits at
/// `start + i * step` milliseconds (`start` defaults to 0); the keymode is the row width.
///
/// ```ignore
/// let chart = chart![step = 100;
///     "[..x...",
///     "|..x...",
///     "]......",
/// ];
/// ```
#[macro_export]
macro_rules! chart {
    (step = $step:expr; $($row:literal),+ $(,)?) => {
        $crate::chart![step = $step, start = 0; $($row),+]
    };
    (step = $step:expr, start = $start:expr; $($row:literal),+ $(,)?) => {
        match $crate::testkit::chart_from_rows($start, $step, &[$($row),+]) {
            Ok(chart) => chart,
            Err(err) => panic!("chart!: {err}"),
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DslError {
    #[error("a chart needs at least one row")]
    Empty,
    #[error("step must be positive, got {0} ms")]
    NonPositiveStep(i32),
    #[error("row width {0} is not a 1-16 column keymode")]
    Keymode(usize),
    #[error("row {row} has {found} columns, expected {expected}")]
    Width {
        row: usize,
        expected: usize,
        found: usize,
    },
    #[error("row {row} column {col}: unknown symbol {symbol:?}")]
    UnknownSymbol {
        row: usize,
        col: usize,
        symbol: char,
    },
    #[error("row {row} column {col}: LN body or tail without a head")]
    BodyOutsideLn { row: usize, col: usize },
    #[error("row {row} column {col}: an LN is open, expected `|` or `]`")]
    NoteInsideLn { row: usize, col: usize },
    #[error("column {col}: LN never closed")]
    UnclosedLn { col: usize },
}

/// The function behind `chart!`, for callers that want the error instead of a panic.
pub fn chart_from_rows(start_ms: i32, step_ms: i32, rows: &[&str]) -> Result<Chart, DslError> {
    let first = rows.first().ok_or(DslError::Empty)?;
    if step_ms <= 0 {
        return Err(DslError::NonPositiveStep(step_ms));
    }
    let width = first.chars().count();
    let keymode = u8::try_from(width)
        .ok()
        .and_then(|w| Keymode::new(w).ok())
        .ok_or(DslError::Keymode(width))?;

    let mut open: Vec<Option<TimeUs>> = vec![None; width];
    let mut notes = Vec::new();
    for (row, line) in rows.iter().enumerate() {
        let found = line.chars().count();
        if found != width {
            return Err(DslError::Width {
                row,
                expected: width,
                found,
            });
        }
        let ms = i64::from(start_ms) + row as i64 * i64::from(step_ms);
        let t = TimeUs(ms * 1000);
        for ((col, symbol), slot) in line.chars().enumerate().zip(open.iter_mut()) {
            let note_col = col as u8;
            match (symbol, *slot) {
                ('.', None) | ('|', Some(_)) => {}
                ('x', None) => notes.push(Note {
                    t,
                    col: note_col,
                    kind: NoteKind::Tap,
                }),
                ('[', None) => *slot = Some(t),
                (']', Some(head)) => {
                    notes.push(Note {
                        t: head,
                        col: note_col,
                        kind: NoteKind::Hold { end: t },
                    });
                    *slot = None;
                }
                ('|' | ']', None) => return Err(DslError::BodyOutsideLn { row, col }),
                ('.' | 'x' | '[', Some(_)) => return Err(DslError::NoteInsideLn { row, col }),
                (symbol, _) => return Err(DslError::UnknownSymbol { row, col, symbol }),
            }
        }
    }
    if let Some(col) = open.iter().position(Option::is_some) {
        return Err(DslError::UnclosedLn { col });
    }

    // One symbol per cell and a positive step cannot draw duplicates or overlaps.
    let mut diags = Diagnostics::new();
    Ok(Chart::from_notes(
        keymode,
        ChartMeta::default(),
        Vec::new(),
        notes,
        &mut diags,
    ))
}

/// Every invariant a `Chart` promises; `Err` names the first violation. Shared by this crate's
/// property tests and downstream fixture tests.
pub fn check_invariants(chart: &Chart) -> Result<(), String> {
    let full = ColMask::full(chart.keymode());
    let rows = chart.rows();
    if let Some(w) = rows.windows(2).find(|w| w[0].t >= w[1].t) {
        return Err(format!("rows not strictly increasing at {:?}", w[1].t));
    }
    for row in rows {
        let (tap, head, tail) = (row.tap.bits(), row.ln_head.bits(), row.ln_tail.bits());
        let held = chart.hold_mask(row.t).bits();
        if (tap | head | tail) & !full.bits() != 0 {
            return Err(format!("row {:?} has a column outside the keymode", row.t));
        }
        if tap & head != 0 || tap & tail != 0 || head & tail != 0 {
            return Err(format!("row {:?} masks overlap", row.t));
        }
        if (tap | head | tail) & held != 0 {
            return Err(format!("row {:?} has an event inside an LN body", row.t));
        }
        if tap | head | tail == 0 {
            return Err(format!("row {:?} is empty", row.t));
        }
    }

    let lns = chart.ln_pairs();
    if lns.windows(2).any(|w| w[0] > w[1]) {
        return Err("ln_pairs not sorted".to_owned());
    }
    if let Some(ln) = lns.iter().find(|ln| ln.head >= ln.tail) {
        return Err(format!("LN {ln:?} has head >= tail"));
    }
    for col in 0..chart.keymode().columns() {
        let mut spans: Vec<_> = lns.iter().filter(|ln| ln.col == col).collect();
        spans.sort();
        if let Some(w) = spans.windows(2).find(|w| w[1].head <= w[0].tail) {
            return Err(format!(
                "column {col}: LNs {:?} and {:?} overlap",
                w[0], w[1]
            ));
        }
    }
    let row_at = |t| {
        rows.binary_search_by_key(&t, |r: &crate::model::Row| r.t)
            .ok()
            .map(|i| rows[i])
    };
    for ln in lns {
        let head_ok = row_at(ln.head).is_some_and(|r| r.ln_head.contains(ln.col));
        let tail_ok = row_at(ln.tail).is_some_and(|r| r.ln_tail.contains(ln.col));
        if !head_ok || !tail_ok {
            return Err(format!("LN {ln:?} has no matching head or tail row"));
        }
    }
    let heads: u32 = rows.iter().map(|r| r.ln_head.len()).sum();
    let tails: u32 = rows.iter().map(|r| r.ln_tail.len()).sum();
    if heads as usize != lns.len() || tails as usize != lns.len() {
        return Err(format!(
            "{heads} heads and {tails} tails for {} LNs",
            lns.len()
        ));
    }
    Ok(())
}

/// One line per row, `t_us` right-aligned, then the DSL symbols (`|` from `hold_mask`). Rows
/// only exist at events, so pure-body DSL rows do not reappear.
pub fn render_rows(chart: &Chart) -> String {
    let mut out = String::new();
    for row in chart.rows() {
        let held = chart.hold_mask(row.t);
        let cells: String = (0..chart.keymode().columns())
            .map(|col| {
                if row.tap.contains(col) {
                    'x'
                } else if row.ln_head.contains(col) {
                    '['
                } else if row.ln_tail.contains(col) {
                    ']'
                } else if held.contains(col) {
                    '|'
                } else {
                    '.'
                }
            })
            .collect();
        out.push_str(&format!("{:>10} {cells}\n", row.t.0));
    }
    out
}

/// Builds `.osu` text for decoder tests. Starts from a valid v14 osu!mania file with no timing
/// lines and no objects; `general`, `metadata` and `difficulty` replace or append a key.
#[derive(Debug, Clone)]
pub struct OsuText {
    keys: u8,
    version: bool,
    sections: Vec<(&'static str, Vec<(String, String)>)>,
    timing: Vec<String>,
    hit_objects: Vec<String>,
}

impl OsuText {
    pub fn mania(keys: u8) -> Self {
        let pairs = |kv: &[(&str, &str)]| -> Vec<(String, String)> {
            kv.iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect()
        };
        let keys_text = keys.to_string();
        Self {
            keys,
            version: true,
            sections: vec![
                (
                    "General",
                    pairs(&[("AudioFilename", "audio.mp3"), ("Mode", "3")]),
                ),
                (
                    "Metadata",
                    pairs(&[
                        ("Title", "Synthetic"),
                        ("Artist", "wolluf"),
                        ("Creator", "wolluf"),
                        ("Version", "Test"),
                        ("BeatmapID", "0"),
                        ("BeatmapSetID", "-1"),
                    ]),
                ),
                (
                    "Difficulty",
                    pairs(&[
                        ("HPDrainRate", "8"),
                        ("CircleSize", &keys_text),
                        ("OverallDifficulty", "8"),
                    ]),
                ),
            ],
            timing: Vec::new(),
            hit_objects: Vec::new(),
        }
    }

    /// Emits the chart's objects, timing and metadata, so decoding the text gives the chart back.
    pub fn from_chart(chart: &Chart) -> Self {
        let meta = chart.meta();
        let id = |v: Option<u32>| v.map_or_else(|| "-1".to_owned(), |v| v.to_string());
        let mut text = Self::mania(chart.keymode().columns())
            .general("AudioFilename", &meta.audio_filename)
            .metadata("Title", &meta.title)
            .metadata("Artist", &meta.artist)
            .metadata("Creator", &meta.creator)
            .metadata("Version", &meta.version)
            .metadata("BeatmapID", &id(meta.beatmap_id))
            .metadata("BeatmapSetID", &id(meta.set_id))
            .difficulty("OverallDifficulty", &meta.od.to_string())
            .difficulty("HPDrainRate", &meta.hp.to_string());
        for point in chart.timing() {
            text = match point.kind {
                TimingKind::Uninherited { beat_len_ms, meter } => text.timing_line(&format!(
                    "{},{beat_len_ms},{meter},1,0,100,1,0",
                    ms_text(point.t)
                )),
                TimingKind::Inherited { sv } => text.timing_line(&format!(
                    "{},{},4,1,0,100,0,0",
                    ms_text(point.t),
                    -100.0 / sv
                )),
            };
        }
        for row in chart.rows() {
            for col in row.tap.iter() {
                text = text.tap(col, row.t);
            }
        }
        for ln in chart.ln_pairs() {
            text = text.hold(ln.col, ln.head, ln.tail);
        }
        text
    }

    pub fn general(self, key: &str, value: &str) -> Self {
        self.set("General", key, value)
    }

    pub fn metadata(self, key: &str, value: &str) -> Self {
        self.set("Metadata", key, value)
    }

    pub fn difficulty(self, key: &str, value: &str) -> Self {
        self.set("Difficulty", key, value)
    }

    pub fn remove(mut self, section: &str, key: &str) -> Self {
        if let Some((_, pairs)) = self.sections.iter_mut().find(|(s, _)| *s == section) {
            pairs.retain(|(k, _)| k != key);
        }
        self
    }

    pub fn without_version(mut self) -> Self {
        self.version = false;
        self
    }

    pub fn timing_line(mut self, line: &str) -> Self {
        self.timing.push(line.to_owned());
        self
    }

    pub fn hit_object(mut self, line: &str) -> Self {
        self.hit_objects.push(line.to_owned());
        self
    }

    pub fn tap(self, col: u8, t: TimeUs) -> Self {
        let x = self.column_x(col);
        self.hit_object(&format!("{x},192,{},1,0,0:0:0:0:", ms_text(t)))
    }

    pub fn hold(self, col: u8, head: TimeUs, tail: TimeUs) -> Self {
        let x = self.column_x(col);
        self.hit_object(&format!(
            "{x},192,{},128,0,{}:0:0:0:0:",
            ms_text(head),
            ms_text(tail)
        ))
    }

    pub fn build(&self) -> String {
        let mut out = String::new();
        if self.version {
            out.push_str("osu file format v14\n\n");
        }
        for (section, pairs) in &self.sections {
            out.push_str(&format!("[{section}]\n"));
            for (k, v) in pairs {
                out.push_str(&format!("{k}: {v}\n"));
            }
            out.push('\n');
        }
        out.push_str("[TimingPoints]\n");
        for line in &self.timing {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("\n[HitObjects]\n");
        for line in &self.hit_objects {
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    /// Unknown sections are ignored: only the three header sections hold keys.
    fn set(mut self, section: &str, key: &str, value: &str) -> Self {
        if let Some((_, pairs)) = self.sections.iter_mut().find(|(s, _)| *s == section) {
            match pairs.iter_mut().find(|(k, _)| k == key) {
                Some(pair) => pair.1 = value.to_owned(),
                None => pairs.push((key.to_owned(), value.to_owned())),
            }
        }
        self
    }

    /// The column's centre, as stable's editor places notes.
    fn column_x(&self, col: u8) -> u32 {
        (2 * u32::from(col) + 1) * 256 / u32::from(self.keys.max(1))
    }
}

/// Exact decimal milliseconds: at most three fraction digits, so parsing back is lossless.
fn ms_text(t: TimeUs) -> String {
    let sign = if t.0 < 0 { "-" } else { "" };
    let abs = t.0.unsigned_abs();
    let (whole, frac) = (abs / 1000, abs % 1000);
    if frac == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{frac:03}")
    }
}

#[cfg(test)]
mod tests {
    use wolluf_core::{ColMask, Keymode, TimeUs};

    use super::*;
    use crate::decode::{ChartDecoder, OsuDecoder};
    use crate::model::LnPair;

    fn mask(cols: &[u8]) -> ColMask {
        ColMask::from_cols(Keymode::K7, cols.iter().copied()).unwrap()
    }

    #[test]
    fn dsl_rows_are_spaced_by_step_from_the_top() {
        let chart = crate::chart![step = 100;
            "x......",
            ".......",
            ".x.x...",
        ];
        assert_eq!(chart.keymode(), Keymode::K7);
        let rows: Vec<(TimeUs, ColMask)> = chart.rows().iter().map(|r| (r.t, r.tap)).collect();
        assert_eq!(
            rows,
            vec![
                (TimeUs::from_ms(0), mask(&[0])),
                (TimeUs::from_ms(200), mask(&[1, 3]))
            ]
        );
    }

    #[test]
    fn dsl_start_offset_and_keymode_from_width() {
        let chart = crate::chart![step = 50, start = 1000;
            "x..x",
            ".xx.",
        ];
        assert_eq!(chart.keymode(), Keymode::K4);
        assert_eq!(chart.rows()[0].t, TimeUs::from_ms(1000));
        assert_eq!(chart.rows()[1].t, TimeUs::from_ms(1050));
    }

    #[test]
    fn dsl_long_notes() {
        let chart = crate::chart![step = 100;
            "[.....x",
            "|[.....",
            "|]....x",
            "]......",
        ];
        assert_eq!(
            chart.ln_pairs(),
            &[
                LnPair {
                    head: TimeUs::from_ms(0),
                    tail: TimeUs::from_ms(300),
                    col: 0
                },
                LnPair {
                    head: TimeUs::from_ms(100),
                    tail: TimeUs::from_ms(200),
                    col: 1
                },
            ]
        );
        assert_eq!(chart.hold_mask(TimeUs::from_ms(150)), mask(&[0, 1]));
        assert_eq!(chart.rows()[2].ln_tail, mask(&[1]));
        assert_eq!(chart.rows()[2].tap, mask(&[6]));
    }

    #[test]
    fn dsl_rejects_malformed_drawings() {
        let bad: [(&[&str], DslError); 8] = [
            (&[], DslError::Empty),
            (
                &["x..", "x."],
                DslError::Width {
                    row: 1,
                    expected: 3,
                    found: 2,
                },
            ),
            (&["x................"], DslError::Keymode(17)),
            (
                &["x.o"],
                DslError::UnknownSymbol {
                    row: 0,
                    col: 2,
                    symbol: 'o',
                },
            ),
            (&["|.."], DslError::BodyOutsideLn { row: 0, col: 0 }),
            (&["]"], DslError::BodyOutsideLn { row: 0, col: 0 }),
            (&["[.", "x."], DslError::NoteInsideLn { row: 1, col: 0 }),
            (&[".[", ".|"], DslError::UnclosedLn { col: 1 }),
        ];
        for (rows, expected) in bad {
            assert_eq!(
                chart_from_rows(0, 100, rows).unwrap_err(),
                expected,
                "{rows:?}"
            );
        }
        assert_eq!(
            chart_from_rows(0, 0, &["x"]).unwrap_err(),
            DslError::NonPositiveStep(0)
        );
        assert_eq!(
            chart_from_rows(0, 100, &["[", "."]).unwrap_err(),
            DslError::NoteInsideLn { row: 1, col: 0 }
        );
    }

    #[test]
    fn render_rows_draws_events_and_bodies() {
        let chart = crate::chart![step = 100;
            "[..x...",
            "|[.....",
            "]|.....",
            ".]....x",
        ];
        assert_eq!(
            render_rows(&chart),
            "         0 [..x...\n    100000 |[.....\n    200000 ]|.....\n    300000 .]....x\n"
        );
    }

    #[test]
    fn dsl_chart_round_trips_through_osu_text() {
        let chart = crate::chart![step = 125, start = 500;
            "[..x..[",
            "|.x.x.|",
            "|x...x]",
            "]..x...",
            "xxxxxxx",
        ];
        let decoded = OsuDecoder
            .decode(OsuText::from_chart(&chart).build().as_bytes())
            .unwrap();
        assert!(decoded.diagnostics.is_empty(), "{:?}", decoded.diagnostics);
        assert_eq!(decoded.chart.rows(), chart.rows());
        assert_eq!(decoded.chart.ln_pairs(), chart.ln_pairs());
        assert_eq!(decoded.chart.keymode(), chart.keymode());
    }
}
