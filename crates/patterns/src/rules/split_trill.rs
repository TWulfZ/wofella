//! `regular.stream.split_trill`: two disjoint chords alternating, stream-fast, at least
//! `split_trill_min_rows` rows, each chord holding at least one note on the left hand and one
//! on the right hand of the user's layout, so both hands play every row (osu! wiki
//! Beatmap/Pattern/osu!mania/Trill). Single notes never qualify. A note on the `Hand::Both`
//! column belongs to neither hand (ADR 0017). Interlude prelude `Chordstream_4K.SPLITTRILL`
//! (MIT, see NOTICE) gives the 3-row minimum.

use wolluf_core::{Keymode, PatternId};

use super::common::{alternations, rows_of, saturating, span_candidate};
use crate::params::PatternParams;
use crate::rule::{Candidate, PatternRule};
use crate::view::ChartView;

pub(super) const ID: PatternId = PatternId::from_static("regular.stream.split_trill");

pub struct SplitTrill;

impl PatternRule for SplitTrill {
    fn id(&self) -> PatternId {
        ID
    }

    fn version(&self) -> u32 {
        1
    }

    fn supports(&self, _keymode: Keymode) -> bool {
        true
    }

    /// Strength: `saturating(rows, split_trill_min_rows)`.
    fn detect(&self, view: &ChartView<'_>, params: &PatternParams) -> Vec<Candidate> {
        let p = &params.stream;
        let rows = view.rows();
        let both_hands =
            |r: &crate::view::RowFeat| !r.hands.left.is_empty() && !r.hands.right.is_empty();
        let mut found: Vec<Candidate> = alternations(rows, p, both_hands)
            .iter()
            .filter_map(|run| {
                let span = rows_of(rows, run);
                if u32::try_from(span.len()).map_or(true, |n| n < p.split_trill_min_rows) {
                    return None;
                }
                span_candidate(
                    ID,
                    view.keymode(),
                    &span,
                    saturating(span.len(), p.split_trill_min_rows),
                )
            })
            .collect();
        found.sort();
        found
    }
}

#[cfg(test)]
mod tests {
    use wolluf_chart::chart;
    use wolluf_chart::testkit::chart_from_rows;

    use super::*;
    use crate::rules::testkit::{cand, detect, detect_with, layout};

    #[test]
    fn chords_spanning_both_hands_alternating_are_a_split_trill() {
        let chart = chart![step = 100; "x.x.x.x", ".x.x.x.", "x.x.x.x", ".x.x.x."];
        for id in ["k7.313_right_thumb", "k7.313_left_thumb", "k7.both_thumbs"] {
            assert_eq!(
                detect_with(&SplitTrill, &chart, &layout(id)),
                [cand(ID, 0, 300, &[0, 1, 2, 3, 4, 5, 6], 500)],
                "{id}"
            );
        }
        let short_chords = chart![step = 100; "x...x..", ".x...x.", "x...x..", ".x...x."];
        assert_eq!(
            detect(&SplitTrill, &short_chords),
            [cand(ID, 0, 300, &[0, 1, 4, 5], 500)]
        );
    }

    #[test]
    fn single_notes_never_split() {
        for cols in [["...x...", "....x.."], ["..x....", "...x..."]] {
            let chart = chart_from_rows(0, 100, &[cols[0], cols[1], cols[0], cols[1]]).unwrap();
            for id in ["k7.313_right_thumb", "k7.313_left_thumb", "k7.both_thumbs"] {
                assert!(
                    detect_with(&SplitTrill, &chart, &layout(id)).is_empty(),
                    "{id} {cols:?}"
                );
            }
        }
    }

    #[test]
    fn a_one_hand_chord_on_either_side_is_not_split() {
        let jumptrill = chart![step = 100; "xxx....", "....xxx", "xxx....", "....xxx"];
        assert!(detect(&SplitTrill, &jumptrill).is_empty());
        let mixed = chart![step = 100; "xxxx...", "....xxx", "xxxx...", "....xxx"];
        assert!(detect(&SplitTrill, &mixed).is_empty());
    }

    #[test]
    fn a_thumb_note_on_the_shared_column_belongs_to_neither_hand() {
        let chart = chart![step = 100; "x..x...", ".x...x.", "x..x...", ".x...x."];
        assert_eq!(
            detect_with(&SplitTrill, &chart, &layout("k7.313_right_thumb")),
            [cand(ID, 0, 300, &[0, 1, 3, 5], 500)]
        );
        assert!(detect_with(&SplitTrill, &chart, &layout("k7.both_thumbs")).is_empty());
    }

    #[test]
    fn near_misses() {
        let short = chart![step = 100; "x.x.x.x", ".x.x.x.", "x.x.x.x"];
        assert!(detect(&SplitTrill, &short).is_empty());
        // A split-chord stair turning around is an `a b a`, not a trill.
        let stair = chart![step = 100; "x...x..", ".x...x.", "..x...x", ".x...x."];
        assert!(detect(&SplitTrill, &stair).is_empty());
        let jacked = chart![step = 100; "x...x..", "x....x.", "x...x.."];
        assert!(detect(&SplitTrill, &jacked).is_empty());
    }
}
