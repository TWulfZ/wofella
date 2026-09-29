//! The normalized chart: one row per distinct instant with tap, LN head and LN tail masks, plus
//! the LN pairs and timing lines. Every decoder and the `chart!` DSL build it through
//! [`Chart::from_notes`], so the row invariants hold for all of them.

use std::collections::BTreeMap;

use wolluf_core::{ColMask, Keymode, TimeUs};

use crate::diag::{DiagCode, Diagnostics};

/// One object as a decoder read it, before normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Note {
    pub t: TimeUs,
    pub col: u8,
    pub kind: NoteKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    Tap,
    Hold { end: TimeUs },
}

/// Masks are pairwise disjoint: a column has at most one event per instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub t: TimeUs,
    pub tap: ColMask,
    pub ln_head: ColMask,
    pub ln_tail: ColMask,
}

/// `head < tail`, and LNs of one column never overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LnPair {
    pub head: TimeUs,
    pub tail: TimeUs,
    pub col: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimingPoint {
    pub t: TimeUs,
    pub kind: TimingKind,
}

/// Raw timing lines, kept unclamped and unmerged so drills can re-emit them (architecture §3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimingKind {
    /// A red line.
    Uninherited { beat_len_ms: f64, meter: u32 },
    /// A green line; `sv` is the scroll-speed multiplier.
    Inherited { sv: f64 },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChartMeta {
    pub title: String,
    pub artist: String,
    pub version: String,
    pub creator: String,
    pub set_id: Option<u32>,
    pub beatmap_id: Option<u32>,
    pub od: f32,
    pub hp: f32,
    pub audio_filename: String,
    /// `None` when the file has no `osu file format v` header line.
    pub format_version: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    keymode: Keymode,
    meta: ChartMeta,
    timing: Vec<TimingPoint>,
    rows: Vec<Row>,
    ln_pairs: Vec<LnPair>,
    /// Per column, the LN spans sorted by head, for `hold_mask`.
    spans_by_col: Vec<Vec<(TimeUs, TimeUs)>>,
}

impl Chart {
    /// Normalizes decoded notes. Oddities are reported, never fatal:
    /// - a column outside the keymode is dropped;
    /// - a hold whose tail is not after its head becomes a tap;
    /// - of several notes at the same column and time, one is kept, preferring the longest hold;
    /// - a note that starts inside an earlier LN of its column (up to and including the tail) is
    ///   dropped.
    pub fn from_notes(
        keymode: Keymode,
        meta: ChartMeta,
        mut timing: Vec<TimingPoint>,
        notes: Vec<Note>,
        diags: &mut Diagnostics,
    ) -> Self {
        timing.sort_by_key(|p| p.t);

        let mut clean: Vec<(ColMask, Note)> = Vec::with_capacity(notes.len());
        for note in notes {
            let Ok(bit) = ColMask::single(keymode, note.col) else {
                diags.push(
                    DiagCode::ColumnOutOfRange,
                    format!("col={} t_us={}", note.col, note.t.0),
                );
                continue;
            };
            let kind = match note.kind {
                NoteKind::Hold { end } if end <= note.t => {
                    diags.push(
                        DiagCode::LnTailNotAfterHead,
                        format!("col={} head_us={} tail_us={}", note.col, note.t.0, end.0),
                    );
                    NoteKind::Tap
                }
                kind => kind,
            };
            clean.push((bit, Note { kind, ..note }));
        }

        // Within a column and instant the longest hold sorts first, so the sweep keeps it.
        clean.sort_by(|(_, a), (_, b)| {
            a.col
                .cmp(&b.col)
                .then(a.t.cmp(&b.t))
                .then(hold_end(b).cmp(&hold_end(a)))
        });

        let mut rows: BTreeMap<TimeUs, Row> = BTreeMap::new();
        let mut ln_pairs = Vec::new();
        let mut spans_by_col = vec![Vec::new(); usize::from(keymode.columns())];
        let mut last: Option<(u8, TimeUs, TimeUs)> = None;
        for (bit, note) in clean {
            if let Some((col, start, end)) = last
                && col == note.col
                && note.t <= end
            {
                let code = if note.t == start {
                    DiagCode::DuplicateNote
                } else {
                    DiagCode::OverlappingNote
                };
                diags.push(code, format!("col={} t_us={}", note.col, note.t.0));
                continue;
            }
            let head_row = row_at(&mut rows, note.t);
            match note.kind {
                NoteKind::Tap => {
                    head_row.tap = union(keymode, head_row.tap, bit);
                    last = Some((note.col, note.t, note.t));
                }
                NoteKind::Hold { end } => {
                    head_row.ln_head = union(keymode, head_row.ln_head, bit);
                    let tail_row = row_at(&mut rows, end);
                    tail_row.ln_tail = union(keymode, tail_row.ln_tail, bit);
                    ln_pairs.push(LnPair {
                        head: note.t,
                        tail: end,
                        col: note.col,
                    });
                    if let Some(spans) = spans_by_col.get_mut(usize::from(note.col)) {
                        spans.push((note.t, end));
                    }
                    last = Some((note.col, note.t, end));
                }
            }
        }
        ln_pairs.sort();

        Self {
            keymode,
            meta,
            timing,
            rows: rows.into_values().collect(),
            ln_pairs,
            spans_by_col,
        }
    }

    pub fn keymode(&self) -> Keymode {
        self.keymode
    }

    pub fn meta(&self) -> &ChartMeta {
        &self.meta
    }

    /// Sorted by time; lines at the same instant keep their file order.
    pub fn timing(&self) -> &[TimingPoint] {
        &self.timing
    }

    /// Strictly increasing `t`.
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Sorted by `(head, tail, col)`.
    pub fn ln_pairs(&self) -> &[LnPair] {
        &self.ln_pairs
    }

    /// Columns whose LN body spans `t`, i.e. `head < t < tail`. Heads and tails at `t` are
    /// already in the row, so this mask never overlaps a row's masks.
    pub fn hold_mask(&self, t: TimeUs) -> ColMask {
        let held = self
            .spans_by_col
            .iter()
            .enumerate()
            .filter(|(_, spans)| {
                let after = spans.partition_point(|&(head, _)| head < t);
                after
                    .checked_sub(1)
                    .and_then(|i| spans.get(i))
                    .is_some_and(|&(_, tail)| t < tail)
            })
            .filter_map(|(col, _)| u8::try_from(col).ok());
        ColMask::from_cols(self.keymode, held).unwrap_or_default()
    }
}

fn hold_end(note: &Note) -> Option<TimeUs> {
    match note.kind {
        NoteKind::Tap => None,
        NoteKind::Hold { end } => Some(end),
    }
}

fn row_at(rows: &mut BTreeMap<TimeUs, Row>, t: TimeUs) -> &mut Row {
    rows.entry(t).or_insert(Row {
        t,
        tap: ColMask::EMPTY,
        ln_head: ColMask::EMPTY,
        ln_tail: ColMask::EMPTY,
    })
}

/// Both masks were validated against `keymode`, so the fallback is unreachable; core has no
/// infallible union.
fn union(keymode: Keymode, a: ColMask, b: ColMask) -> ColMask {
    ColMask::from_bits(keymode, a.bits() | b.bits()).unwrap_or(a)
}

#[cfg(test)]
mod tests {
    use wolluf_core::{ColMask, Keymode, TimeUs};

    use super::*;
    use crate::diag::{DiagCode, Diagnostics};

    const K7: Keymode = Keymode::K7;

    fn ms(v: i32) -> TimeUs {
        TimeUs::from_ms(v)
    }

    fn tap(t: i32, col: u8) -> Note {
        Note {
            t: ms(t),
            col,
            kind: NoteKind::Tap,
        }
    }

    fn hold(t: i32, end: i32, col: u8) -> Note {
        Note {
            t: ms(t),
            col,
            kind: NoteKind::Hold { end: ms(end) },
        }
    }

    fn mask(cols: &[u8]) -> ColMask {
        ColMask::from_cols(K7, cols.iter().copied()).unwrap()
    }

    fn build(notes: Vec<Note>) -> (Chart, Diagnostics) {
        let mut diags = Diagnostics::new();
        let chart = Chart::from_notes(K7, ChartMeta::default(), Vec::new(), notes, &mut diags);
        (chart, diags)
    }

    #[test]
    fn rows_merge_same_instant_and_sort_by_time() {
        let (chart, diags) = build(vec![tap(200, 1), tap(100, 3), tap(100, 0)]);
        assert!(diags.is_empty());
        let rows = chart.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].t, ms(100));
        assert_eq!(rows[0].tap, mask(&[0, 3]));
        assert_eq!(rows[1].t, ms(200));
        assert_eq!(rows[1].tap, mask(&[1]));
    }

    #[test]
    fn hold_produces_head_and_tail_rows_and_a_pair() {
        let (chart, diags) = build(vec![hold(100, 400, 2), tap(400, 5)]);
        assert!(diags.is_empty());
        let rows = chart.rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].ln_head, mask(&[2]));
        assert_eq!(rows[1].ln_tail, mask(&[2]));
        assert_eq!(rows[1].tap, mask(&[5]));
        assert_eq!(
            chart.ln_pairs(),
            &[LnPair {
                head: ms(100),
                tail: ms(400),
                col: 2
            }]
        );
    }

    #[test]
    fn duplicate_note_is_reported_and_one_is_kept() {
        let (chart, diags) = build(vec![tap(100, 1), tap(100, 1)]);
        assert_eq!(diags.codes(), vec![DiagCode::DuplicateNote]);
        assert_eq!(chart.rows().len(), 1);
        assert_eq!(chart.rows()[0].tap, mask(&[1]));
    }

    #[test]
    fn duplicate_prefers_the_hold() {
        let (chart, diags) = build(vec![tap(100, 1), hold(100, 300, 1)]);
        assert_eq!(diags.codes(), vec![DiagCode::DuplicateNote]);
        assert_eq!(chart.ln_pairs().len(), 1);
        assert_eq!(chart.rows()[0].tap, ColMask::EMPTY);
    }

    #[test]
    fn hold_with_tail_not_after_head_degrades_to_tap() {
        let (chart, diags) = build(vec![hold(100, 100, 0), hold(200, 150, 1)]);
        assert_eq!(
            diags.codes(),
            vec![DiagCode::LnTailNotAfterHead, DiagCode::LnTailNotAfterHead]
        );
        assert!(chart.ln_pairs().is_empty());
        assert_eq!(chart.rows()[0].tap, mask(&[0]));
        assert_eq!(chart.rows()[1].tap, mask(&[1]));
    }

    #[test]
    fn note_inside_an_ln_body_of_its_column_is_dropped() {
        let (chart, diags) = build(vec![
            hold(100, 400, 3),
            tap(250, 3),
            tap(400, 3),
            tap(250, 4),
        ]);
        assert_eq!(
            diags.codes(),
            vec![DiagCode::OverlappingNote, DiagCode::OverlappingNote]
        );
        let taps: Vec<(TimeUs, ColMask)> = chart.rows().iter().map(|r| (r.t, r.tap)).collect();
        assert_eq!(
            taps,
            vec![
                (ms(100), ColMask::EMPTY),
                (ms(250), mask(&[4])),
                (ms(400), ColMask::EMPTY)
            ]
        );
    }

    #[test]
    fn column_outside_keymode_is_dropped() {
        let (chart, diags) = build(vec![tap(100, 7), tap(100, 6)]);
        assert_eq!(diags.codes(), vec![DiagCode::ColumnOutOfRange]);
        assert_eq!(chart.rows()[0].tap, mask(&[6]));
    }

    #[test]
    fn hold_mask_is_the_open_body_interval() {
        let (chart, _) = build(vec![hold(100, 400, 1), hold(200, 300, 5), tap(250, 0)]);
        assert_eq!(chart.hold_mask(ms(100)), ColMask::EMPTY);
        assert_eq!(chart.hold_mask(ms(150)), mask(&[1]));
        assert_eq!(chart.hold_mask(ms(250)), mask(&[1, 5]));
        assert_eq!(chart.hold_mask(ms(300)), mask(&[1]));
        assert_eq!(chart.hold_mask(ms(400)), ColMask::EMPTY);
        assert_eq!(chart.hold_mask(ms(50)), ColMask::EMPTY);
    }

    #[test]
    fn empty_chart_has_no_rows() {
        let (chart, diags) = build(Vec::new());
        assert!(diags.is_empty());
        assert!(chart.rows().is_empty());
        assert_eq!(chart.keymode(), K7);
    }
}

#[cfg(test)]
pub(crate) mod props {
    use proptest::prelude::*;
    use wolluf_core::{Keymode, TimeUs};

    use super::*;
    use crate::diag::Diagnostics;
    use crate::testkit::check_invariants;

    pub(crate) fn arb_notes(max_col: u8) -> impl Strategy<Value = Vec<Note>> {
        let note =
            (0i64..2_000, 0..max_col, prop::option::of(-50i64..400)).prop_map(|(t, col, len)| {
                let t = TimeUs(t * 500);
                let kind = match len {
                    None => NoteKind::Tap,
                    Some(len) => NoteKind::Hold {
                        end: TimeUs(t.0 + len * 500),
                    },
                };
                Note { t, col, kind }
            });
        prop::collection::vec(note, 0..120)
    }

    proptest! {
        #[test]
        fn normalized_rows_hold_invariants(keys in 1u8..=16, notes in arb_notes(18)) {
            let keymode = Keymode::new(keys).unwrap();
            let mut diags = Diagnostics::new();
            let chart = Chart::from_notes(keymode, ChartMeta::default(), Vec::new(), notes, &mut diags);
            prop_assert_eq!(check_invariants(&chart), Ok(()));
        }

        #[test]
        fn normalization_is_idempotent(notes in arb_notes(7)) {
            let mut diags = Diagnostics::new();
            let once = Chart::from_notes(Keymode::K7, ChartMeta::default(), Vec::new(), notes, &mut diags);
            let mut again_diags = Diagnostics::new();
            let again = Chart::from_notes(
                Keymode::K7,
                ChartMeta::default(),
                Vec::new(),
                notes_of(&once),
                &mut again_diags,
            );
            prop_assert!(again_diags.is_empty(), "{:?}", again_diags);
            prop_assert_eq!(again, once);
        }
    }

    fn notes_of(chart: &Chart) -> Vec<Note> {
        let taps = chart.rows().iter().flat_map(|row| {
            row.tap.iter().map(move |col| Note {
                t: row.t,
                col,
                kind: NoteKind::Tap,
            })
        });
        let holds = chart.ln_pairs().iter().map(|ln| Note {
            t: ln.head,
            col: ln.col,
            kind: NoteKind::Hold { end: ln.tail },
        });
        taps.chain(holds).collect()
    }
}
