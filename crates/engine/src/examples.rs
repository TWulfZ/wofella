//! One canonical synthetic example per pattern id, for the labeller's pattern previews. The
//! charts are synthetic so labelling stays blind (ADR 0018), and a test pins that the engine
//! detects each one as its own pattern, so a preview cannot drift from the rules.

use wolluf_chart::{Chart, ChartMeta, Diagnostics, Note, NoteKind, TimingKind, TimingPoint};
use wolluf_core::{Keymode, PatternId, TimeUs};

#[derive(Debug, Clone, PartialEq)]
pub struct PatternExample {
    pub id: PatternId,
    pub chart: Chart,
    /// `[from, to]` in ms, both inclusive: the characteristic shape a preview draws.
    pub display_ms: (i32, i32),
}

/// In taxonomy order. Ids come from [`crate::taxonomy::k7`].
/// The keymode's examples; empty for a keymode without them (D5: features never match keymodes).
pub fn for_keymode(keymode: Keymode) -> Vec<PatternExample> {
    match keymode {
        Keymode::K7 => k7(),
        _ => Vec::new(),
    }
}

pub fn k7() -> Vec<PatternExample> {
    crate::taxonomy::k7()
        .iter()
        .filter_map(|def| {
            let draft = k7_draft(def.id.as_str())?;
            let chart = draft.chart()?;
            let display_ms = draft.display_ms.or_else(|| span_ms(&chart))?;
            Some(PatternExample {
                id: def.id.clone(),
                chart,
                display_ms,
            })
        })
        .collect()
}

/// 150 BPM: 1/4 rows are 100 ms apart and the segmenter's 4-beat minimum is 1.6 s.
const BEAT_MS: f64 = 400.0;
const QUARTER_MS: i32 = 100;

/// Rows use the `chart!` testkit symbols, earliest first: `.` empty, `x` tap, `[` LN head, `|`
/// LN body, `]` LN tail.
struct Draft {
    /// A red line at 0. Without one, short sections need 2 s instead of 4 beats to segment, and
    /// tech and delay rules read no snap.
    beat_len_ms: Option<f64>,
    rows: Vec<(i32, &'static str)>,
    /// Defaults to the first and last row.
    display_ms: Option<(i32, i32)>,
}

impl Draft {
    fn red(rows: Vec<(i32, &'static str)>) -> Self {
        Self {
            beat_len_ms: Some(BEAT_MS),
            rows,
            display_ms: None,
        }
    }

    fn unruled(rows: Vec<(i32, &'static str)>) -> Self {
        Self {
            beat_len_ms: None,
            rows,
            display_ms: None,
        }
    }

    fn display(self, from_ms: i32, to_ms: i32) -> Self {
        Self {
            display_ms: Some((from_ms, to_ms)),
            ..self
        }
    }

    /// `None` on a malformed drawing, which drops the example and fails the coverage test.
    fn chart(&self) -> Option<Chart> {
        let keymode = Keymode::K7;
        let width = usize::from(keymode.columns());
        let mut open: Vec<Option<TimeUs>> = vec![None; width];
        let mut notes = Vec::new();
        for &(t_ms, line) in &self.rows {
            if line.chars().count() != width {
                return None;
            }
            let t = TimeUs::from_ms(t_ms);
            for ((col, symbol), slot) in (0u8..).zip(line.chars()).zip(open.iter_mut()) {
                match (symbol, *slot) {
                    ('.', None) | ('|', Some(_)) => {}
                    ('x', None) => notes.push(Note {
                        t,
                        col,
                        kind: NoteKind::Tap,
                    }),
                    ('[', None) => *slot = Some(t),
                    (']', Some(head)) => {
                        notes.push(Note {
                            t: head,
                            col,
                            kind: NoteKind::Hold { end: t },
                        });
                        *slot = None;
                    }
                    _ => return None,
                }
            }
        }
        if open.iter().any(Option::is_some) {
            return None;
        }
        let timing = self
            .beat_len_ms
            .map(|beat_len_ms| TimingPoint {
                t: TimeUs(0),
                kind: TimingKind::Uninherited {
                    beat_len_ms,
                    meter: 4,
                },
            })
            .into_iter()
            .collect();
        let mut diags = Diagnostics::new();
        let chart = Chart::from_notes(keymode, ChartMeta::default(), timing, notes, &mut diags);
        diags.is_empty().then_some(chart)
    }
}

fn span_ms(chart: &Chart) -> Option<(i32, i32)> {
    let ms = |t: TimeUs| i32::try_from(t.as_ms_floor()).ok();
    Some((ms(chart.rows().first()?.t)?, ms(chart.rows().last()?.t)?))
}

fn grid(step_ms: i32, rows: &[&'static str]) -> Vec<(i32, &'static str)> {
    (0..)
        .zip(rows)
        .map(|(i, &row)| (i * step_ms, row))
        .collect()
}

fn quarters(pattern: &[&'static str], n: usize) -> Vec<(i32, &'static str)> {
    let rows: Vec<&'static str> = pattern.iter().copied().cycle().take(n).collect();
    grid(QUARTER_MS, &rows)
}

/// Columns tapped one per instant at the given gaps, cycling both lists.
fn taps(start_ms: i32, cols: &[usize], gaps_ms: &[i32], n: usize) -> Vec<(i32, &'static str)> {
    const SINGLE: [&str; 7] = [
        "x......", ".x.....", "..x....", "...x...", "....x..", ".....x.", "......x",
    ];
    let mut t = start_ms;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if let Some(&row) = cols.get(i % cols.len().max(1)).and_then(|&c| SINGLE.get(c)) {
            out.push((t, row));
        }
        t += gaps_ms
            .get(i % gaps_ms.len().max(1))
            .copied()
            .unwrap_or(QUARTER_MS);
    }
    out
}

/// 4.75 beats of 1/4 rows: over the segmenter's 4-beat minimum under the red line.
const STREAM_ROWS: usize = 20;

fn k7_draft(id: &str) -> Option<Draft> {
    let draft = match id {
        // Sparse taps (slower than any stream) around one or more same-column runs: jacks rank
        // last, so any stream around them would take the segment.
        "regular.jack.minijack" => Draft::unruled(grid(
            300,
            &[
                "x......", "..x....", ".....x.", ".....x.", ".x.....", "...x...",
            ],
        )),
        "regular.jack.longjack" => Draft::unruled(grid(
            300,
            &[
                "x......", "......x", "..x....", "..x....", "..x....", "..x....", "....x..",
                ".x.....",
            ],
        )),
        "regular.jack.chordjack" => Draft::unruled(grid(
            150,
            &[
                "xx..xx.", "x..xx.x", "xx.x..x", ".x.xx.x", "xx..x.x", "x.x.xx.", "x.xx.x.",
                "xx.x.x.", ".x.x.xx",
            ],
        )),
        "regular.jack.anchor" => Draft::red(quarters(
            &[
                "x......", "..x....", "x......", "....x..", "x......", "......x", "x......",
                "...x...", "x......", ".x.....", "x......", ".....x.",
            ],
            STREAM_ROWS,
        )),
        // 1/4 against 1/3 gaps of the beat.
        "regular.tech.irregular" => {
            Draft::red(taps(0, &[1, 4, 2, 5, 0, 3, 6, 2], &[100, 133], STREAM_ROWS))
        }
        // Tag-only, like thumb: a single stream owns the segment.
        "regular.tech.hand_imbalance" => Draft::red(quarters(
            &[
                "....x..", "......x", ".....x.", "...x...", "......x", "x......", "....x..",
                ".....x.", "...x...", "......x", "....x..", ".x.....",
            ],
            STREAM_ROWS,
        )),
        "regular.tech.thumb" => Draft::red(quarters(
            &[
                "...x...", "x......", "...x...", ".....x.", ".x.....", "...x...", "......x",
                "...x...", "..x....", "....x..",
            ],
            STREAM_ROWS,
        )),
        // A 1/4 run four times the pace of the taps around it; the lead-in and tail sit
        // outside the preview but set the surrounding pace.
        "regular.speed.burst" => {
            let mut rows = taps(0, &[0, 4, 2, 6, 1, 5], &[400], 6);
            rows.extend(taps(2_400, &[2, 4, 1, 5, 3, 6], &[100], 6));
            rows.extend(taps(3_300, &[0, 4, 2, 6, 1, 5], &[400], 6));
            Draft::red(rows).display(1_600, 3_700)
        }
        "regular.stream.single" => Draft::red(quarters(
            &[
                "x......", "...x...", ".x.....", ".....x.", "..x....", "......x", "....x..",
            ],
            STREAM_ROWS,
        )),
        "regular.stream.jumpstream" => Draft::red(quarters(
            &[
                "x......", "..xx...", "x......", ".x..x..", "..x....", "x....x.", "...x...",
                ".x..x..",
            ],
            STREAM_ROWS,
        )),
        "regular.stream.handstream" => Draft::red(quarters(
            &[
                "x.x.x..", "......x", ".x...x.", "x......", "..x.x.x", ".x.....", "......x",
                ".x.x.x.",
            ],
            STREAM_ROWS,
        )),
        "regular.stream.chordstream_light" => Draft::red(quarters(
            &[
                "xx.....", "....xx.", "..xx...", "x......", ".x...xx", "x.x....", "....x.x",
                "...x...",
            ],
            STREAM_ROWS,
        )),
        "regular.stream.chordstream_dense" => Draft::red(quarters(
            &[
                "x.x.x.x", "...x...", ".xx..xx", "....x..", "x.xx..x", ".....x.", "xx.xx..",
                "......x",
            ],
            STREAM_ROWS,
        )),
        "regular.stream.roll" => Draft::red(quarters(
            &[
                "x......", ".x.....", "..x....", "...x...", "....x..", ".....x.", "......x",
                ".....x.", "....x..", "...x...", "..x....", ".x.....",
            ],
            STREAM_ROWS,
        )),
        "regular.stream.trill" => Draft::red(quarters(&[".x.....", ".....x."], STREAM_ROWS)),
        "regular.stream.jumptrill" => Draft::red(quarters(&[".xx....", "....xx."], STREAM_ROWS)),
        "regular.stream.split_trill" => Draft::red(quarters(&["x....x.", ".x..x.."], STREAM_ROWS)),
        "regular.stream.bracket" => Draft::red(quarters(&["x.x....", ".x....."], STREAM_ROWS)),
        // Each hand plays one note per chord, so no hand brackets.
        "regular.stream.chordtrill" => {
            Draft::red(quarters(&["x...x..", "..x..x.", ".x....x"], STREAM_ROWS))
        }
        // Tag-only, so release timing owns the segment; three columns held at any time keep
        // the coverage over the density floor.
        "ln.general.density" => Draft::red(grid(
            QUARTER_MS,
            &[
                "[......", "|.[....", "|.|.[..", "].|.|.[", ".[].|.|", ".|.[].|", ".|.|.[]",
                "[].|.|.", "|.[].|.", "|.|.[].", "].|.|.[", ".[].|.|", ".|.[].|", ".|.|.[]",
                "[].|.|.", "|.[].|.", "|.|.[].", "].|.|.[", "..].|.|", "....].|", "......]",
            ],
        )),
        "ln.general.chord" => Draft::red(grid(
            QUARTER_MS,
            &[
                "[..[..[", "|..|..|", "]..]..]", ".......", ".[..[..", ".|..|..", ".]..]..",
                ".......", "..[..[.", "..|..|.", "..]..].", ".......", "[.[.[.[", "|.|.|.|",
                "].].].]", ".......", ".[...[.", ".|...|.", ".]...].",
            ],
        )),
        "ln.tech.hybrid" => Draft::red(quarters(
            &[
                "x.....[", "..x...|", ".x....|", "...x..]", "..x...[", "....x.|", "...x..|",
                ".x....]",
            ],
            STREAM_ROWS,
        )),
        "ln.tech.shield" => Draft::red(grid(
            QUARTER_MS,
            &[
                "x......", "[...x..", "|...[..", "]...|..", "....]..", "..x....", "..[...x",
                "..|...[", "..]...|", "......]", "...x...", "...[.x.", "...|.[.", "...].|.",
                ".....].",
            ],
        )),
        // Every column held, each released for one 1/4 gap in turn: the negative of a stream.
        "ln.inverse.gap" => Draft::red(grid(
            QUARTER_MS,
            &[
                "[[[[[[[", "]||||||", "[||]|||", "|||[|]|", "|]|||[|", "|[||||]", "||]|||[",
                "||[|]||", "|]||[||", "|[|||]|", "|||]|[|", "]||[|||", "[|||||]", "||]|||[",
                "||[|]||", "||||[||", "]]]]]]]",
            ],
        )),
        "ln.release.timing" => Draft::red(grid(
            QUARTER_MS,
            &[
                "[[[[...", "||||...", "]|||...", ".]||...", "..]|...", "...]...", "...[[[[",
                "...||||", "...|||]", "...||].", "...|]..", "...]...", "[[[[...", "||||...",
                "]|||...", ".]||...", "..]|...", "...]...", "...[[[[", "...||||", "...|||]",
                "...||].", "...|]..", "...]...",
            ],
        )),
        // 1/4 jumps split 1/16 + 3/16 apart. Moves alternate direction: a roll inside would
        // cut the section into pieces too short to segment.
        "regular.speed.delay" => Draft::red(taps(
            0,
            &[0, 4, 1, 5, 2, 6, 3, 5, 1, 4, 0, 6, 2, 3],
            &[25, 75],
            36,
        )),
        _ => return None,
    };
    Some(draft)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use wolluf_core::{Keymode, TimeUs};
    use wolluf_patterns::PatternParams;

    use super::*;
    use crate::profile::Registry;
    use crate::stage::patterns;

    #[test]
    fn examples_cover_every_profile_taxonomy_exactly_once() {
        for profile in Registry::builtin().profiles() {
            let examples = for_keymode(profile.keymode);
            let ids: Vec<&str> = examples.iter().map(|e| e.id.as_str()).collect();
            let unique: BTreeSet<&str> = ids.iter().copied().collect();
            assert_eq!(unique.len(), ids.len(), "duplicate example ids: {ids:?}");
            let expected: BTreeSet<&str> = profile
                .taxonomy
                .unwrap_or_default()
                .iter()
                .map(|p| p.id.as_str())
                .collect();
            assert_eq!(unique, expected, "{:?}", profile.keymode);
        }
    }

    #[test]
    fn display_spans_fit_a_preview() {
        for ex in k7() {
            let (from, to) = ex.display_ms;
            assert!(
                (1_200..=2_500).contains(&(to - from)),
                "{}: {from}..{to}",
                ex.id.as_str()
            );
        }
    }

    #[test]
    fn every_example_is_detected_as_its_own_pattern() {
        // The layout the app serves the previews under.
        let layout = Registry::builtin().profile(Keymode::K7).unwrap().layout();
        let tag_only = PatternParams::default().segment.tag_only;
        let examples = k7();
        assert!(!examples.is_empty());
        let mut failures = Vec::new();
        for ex in &examples {
            let (from, to) = ex.display_ms;
            let mid = TimeUs::from_ms(from + (to - from) / 2);
            let segments = patterns::run(&ex.chart, &layout).unwrap();
            let covering = segments.iter().find(|s| s.t0 <= mid && mid <= s.t1);
            let ok = covering.is_some_and(|s| {
                if tag_only.contains(&ex.id) {
                    s.secondary.contains(&ex.id)
                } else {
                    s.primary == ex.id
                }
            });
            if !ok {
                let found: Vec<String> = segments
                    .iter()
                    .map(|s| {
                        let tags: Vec<&str> = s.secondary.iter().map(|p| p.as_str()).collect();
                        format!(
                            "{}..{} {} [{}]",
                            s.t0.0 / 1000,
                            s.t1.0 / 1000,
                            s.primary.as_str(),
                            tags.join(",")
                        )
                    })
                    .collect();
                failures.push(format!(
                    "{} (mid {} ms): {found:#?}",
                    ex.id.as_str(),
                    mid.0 / 1000
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
