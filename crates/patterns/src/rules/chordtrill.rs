//! `regular.stream.chordtrill`: chords alternating or moving across columns without jacking
//! (osu! wiki Beatmap/Pattern/osu!mania/Trill; ADR 0017). A run of at least
//! `chordtrill_min_rows` stream-fast press rows of 2+ notes, each sharing no column with the
//! previous one, where each row either belongs to a strict two-chord alternation (`a b a …`)
//! or interleaves with the previous row rather than sitting on one side of it (not an
//! Interlude roll). The moving shape adapts Interlude prelude `Chordstream_7K.BRACKETS` (MIT,
//! see NOTICE: 3 rows, no roll, no jack). Jumptrills and split trills are chordtrills too and
//! outrank it in the segmenter; a one-sided chord move that never repeats is a roll. Reads no
//! hands.

use wolluf_core::{Keymode, PatternId};

use super::common::{Step, alternations, rows_of, saturating, scan, span_candidate, stream_gap_ok};
use crate::params::PatternParams;
use crate::rule::{Candidate, PatternRule};
use crate::view::ChartView;

const CHORD_MIN_NOTES: u32 = 2;
/// `a b a`: the first repeat is what makes two chords an alternation rather than a move.
const ALTERNATION_MIN_ROWS: usize = 3;

pub(super) const ID: PatternId = PatternId::from_static("regular.stream.chordtrill");

pub struct Chordtrill;

impl PatternRule for Chordtrill {
    fn id(&self) -> PatternId {
        ID
    }

    fn version(&self) -> u32 {
        1
    }

    fn supports(&self, _keymode: Keymode) -> bool {
        true
    }

    /// Strength: `saturating(rows, chordtrill_min_rows)`.
    fn detect(&self, view: &ChartView<'_>, params: &PatternParams) -> Vec<Candidate> {
        let p = &params.stream;
        let rows = view.rows();
        let alternating = alternations(rows, p, |r| r.notes >= CHORD_MIN_NOTES)
            .into_iter()
            .filter(|run| run.len() >= ALTERNATION_MIN_ROWS);
        let moving = scan(rows, |run, i| {
            let Some(row) = rows.get(i) else {
                return Step::Break;
            };
            if row.notes < CHORD_MIN_NOTES {
                return Step::Break;
            }
            let linked = row.jacks == 0 && !row.is_roll && stream_gap_ok(row, p);
            if run.is_empty() || !linked {
                Step::Restart
            } else {
                Step::Extend
            }
        });
        let mut found: Vec<Candidate> = merge_overlapping(alternating.chain(moving).collect())
            .iter()
            .filter_map(|run| {
                let span = rows_of(rows, run);
                if u32::try_from(span.len()).map_or(true, |n| n < p.chordtrill_min_rows) {
                    return None;
                }
                span_candidate(
                    ID,
                    view.keymode(),
                    &span,
                    saturating(span.len(), p.chordtrill_min_rows),
                )
            })
            .collect();
        found.sort();
        found
    }
}

/// Unions runs of consecutive press rows that share a row; both sources only link consecutive
/// press rows, so a merged run stays consecutive.
fn merge_overlapping(mut runs: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
    runs.sort_unstable_by_key(|run| run.first().copied());
    let mut merged: Vec<Vec<usize>> = Vec::new();
    for run in runs {
        match merged.last_mut() {
            Some(open) if run.first() <= open.last() => {
                let end = open.last().copied();
                open.extend(run.into_iter().filter(|&i| Some(i) > end));
            }
            _ => merged.push(run),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use wolluf_chart::chart;
    use wolluf_chart::testkit::chart_from_rows;

    use super::*;
    use crate::rules::testkit::{cand, detect, detect_with, layout};

    #[test]
    fn a_two_note_shape_moving_without_jacks_is_a_chordtrill() {
        let chart = chart![step = 100; "x.x....", ".x.x...", "..x.x..", "...x.x."];
        assert_eq!(
            detect(&Chordtrill, &chart),
            [cand(ID, 0, 300, &[0, 1, 2, 3, 4, 5], 666)]
        );
    }

    #[test]
    fn two_chords_alternating_are_a_chordtrill_whatever_their_hands() {
        let cases: [([&str; 2], &[u8]); 5] = [
            (["xxx....", "....xxx"], &[0, 1, 2, 4, 5, 6]),
            (["x.x.x.x", ".x.x.x."], &[0, 1, 2, 3, 4, 5, 6]),
            (["xxxx...", "....xxx"], &[0, 1, 2, 3, 4, 5, 6]),
            (["xx.....", "..xx..."], &[0, 1, 2, 3]),
            (["...xx..", ".....xx"], &[3, 4, 5, 6]),
        ];
        for ([a, b], cols) in cases {
            let chart = chart_from_rows(0, 100, &[a, b, a, b]).unwrap();
            for id in ["k7.313_right_thumb", "k7.313_left_thumb", "k7.both_thumbs"] {
                assert_eq!(
                    detect_with(&Chordtrill, &chart, &layout(id)),
                    [cand(ID, 0, 300, cols, 666)],
                    "{id} {a} {b}"
                );
            }
        }
    }

    #[test]
    fn an_alternation_running_into_a_moving_shape_is_one_chordtrill() {
        let chart = chart![step = 100;
            "xx.....", "..xx...", "xx.....", "..xx...", ".x..x..", "x.x....",
        ];
        assert_eq!(
            detect(&Chordtrill, &chart),
            [cand(ID, 0, 500, &[0, 1, 2, 3, 4], 1000)]
        );
    }

    #[test]
    fn chord_rolls_and_jacks_are_not_chordtrills() {
        let roll = chart![step = 100; "xx.....", "..xx...", "....xx."];
        assert!(detect(&Chordtrill, &roll).is_empty());
        let jacked = chart![step = 100; "x.x....", "..xx...", ".x..x.."];
        assert!(detect(&Chordtrill, &jacked).is_empty());
        let jacked_alternation = chart![step = 100; "xx.....", ".xx....", "xx.....", ".xx...."];
        assert!(detect(&Chordtrill, &jacked_alternation).is_empty());
    }

    #[test]
    fn near_misses() {
        let short = chart![step = 100; "x.x....", ".x.x..."];
        assert!(detect(&Chordtrill, &short).is_empty());
        let single_notes = chart![step = 100; "x......", "....x..", "x......", "....x.."];
        assert!(detect(&Chordtrill, &single_notes).is_empty());
        let slow = chart![step = 251; "xxx....", "....xxx", "xxx...."];
        assert!(detect(&Chordtrill, &slow).is_empty());
    }
}
