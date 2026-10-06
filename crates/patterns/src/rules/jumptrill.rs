//! `regular.stream.jumptrill`: two disjoint chords alternating, stream-fast, at least
//! `jumptrill_min_rows` rows, each chord wholly within one hand of the user's layout and the
//! two chords on different hands, so the hands take turns (osu! wiki Beatmap/Pattern/osu!mania/
//! Trill; Interlude prelude `Chordstream_4K.JUMPTRILL`, MIT, see NOTICE, gives the 4-row
//! minimum). Chords of any size count. A chord on the `Hand::Both` column belongs to neither
//! hand; a one-hand alternation is a bracket, and other chord alternations are chordtrills.

use wolluf_core::{Keymode, PatternId};

use super::common::{alternations, rows_of, saturating, span_candidate};
use crate::params::PatternParams;
use crate::rule::{Candidate, PatternRule};
use crate::view::{ChartView, RowFeat};

pub(super) const ID: PatternId = PatternId::from_static("regular.stream.jumptrill");

/// ADR 0017: both sides are chords (2+ notes); a single note against a chord is not a jumptrill.
const CHORD_MIN_NOTES: u32 = 2;

pub struct Jumptrill;

impl PatternRule for Jumptrill {
    fn id(&self) -> PatternId {
        ID
    }

    fn version(&self) -> u32 {
        1
    }

    fn supports(&self, _keymode: Keymode) -> bool {
        true
    }

    /// Strength: `saturating(rows, jumptrill_min_rows)`.
    fn detect(&self, view: &ChartView<'_>, params: &PatternParams) -> Vec<Candidate> {
        let p = &params.stream;
        let rows = view.rows();
        let hands = view.hand_cols();
        let side = |r: &RowFeat| {
            if r.notes < CHORD_MIN_NOTES {
                None
            } else if r.press.is_subset_of(hands.left) {
                Some(Side::Left)
            } else if r.press.is_subset_of(hands.right) {
                Some(Side::Right)
            } else {
                None
            }
        };
        let mut found: Vec<Candidate> = alternations(rows, p, |r| side(r).is_some())
            .iter()
            .filter_map(|run| {
                let span = rows_of(rows, run);
                if u32::try_from(span.len()).map_or(true, |n| n < p.jumptrill_min_rows) {
                    return None;
                }
                if side(span.first()?)? == side(span.get(1)?)? {
                    return None;
                }
                span_candidate(
                    ID,
                    view.keymode(),
                    &span,
                    saturating(span.len(), p.jumptrill_min_rows),
                )
            })
            .collect();
        found.sort();
        found
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use wolluf_chart::chart;

    use super::*;
    use crate::rules::testkit::{cand, detect, detect_with, layout};

    #[test]
    fn one_hand_chords_taking_turns_between_the_hands_are_a_jumptrill() {
        let chart = chart![step = 100; "xxx....", "....xxx", "xxx....", "....xxx"];
        assert_eq!(
            detect(&Jumptrill, &chart),
            [cand(ID, 0, 300, &[0, 1, 2, 4, 5, 6], 500)]
        );
        let pairs = chart![step = 100; "xx.....", "...xx..", "xx.....", "...xx.."];
        assert_eq!(
            detect(&Jumptrill, &pairs),
            [cand(ID, 0, 300, &[0, 1, 3, 4], 500)]
        );
    }

    #[test]
    fn a_whole_hand_chord_is_a_jumptrill_side() {
        let chart = chart![step = 100; "xxx....", "...xxxx", "xxx....", "...xxxx"];
        assert_eq!(
            detect(&Jumptrill, &chart),
            [cand(ID, 0, 300, &[0, 1, 2, 3, 4, 5, 6], 500)]
        );
    }

    #[test]
    fn split_mixed_and_same_hand_chord_alternations_are_not_jumptrills() {
        let split = chart![step = 100; "x.x.x.x", ".x.x.x.", "x.x.x.x", ".x.x.x."];
        assert!(detect(&Jumptrill, &split).is_empty());
        let mixed = chart![step = 100; "xxxx...", "....xxx", "xxxx...", "....xxx"];
        for id in ["k7.313_right_thumb", "k7.both_thumbs"] {
            assert!(
                detect_with(&Jumptrill, &mixed, &layout(id)).is_empty(),
                "{id}"
            );
        }
        let across = chart![step = 100; "xx.....", "..xx...", "xx.....", "..xx..."];
        assert!(detect(&Jumptrill, &across).is_empty());
        let same_hand = chart![step = 100; "...xx..", ".....xx", "...xx..", ".....xx"];
        assert!(detect(&Jumptrill, &same_hand).is_empty());
    }

    #[test]
    fn single_trills_and_jacking_chords_are_not_jumptrills() {
        let trill = chart![step = 100; "x......", "....x..", "x......", "....x.."];
        assert!(detect(&Jumptrill, &trill).is_empty());
        let jacked = chart![step = 100; "xx.....", ".xx....", "xx.....", ".xx...."];
        assert!(detect(&Jumptrill, &jacked).is_empty());
    }

    #[test]
    fn near_misses() {
        let short = chart![step = 100; "xxx....", "....xxx", "xxx...."];
        assert!(detect(&Jumptrill, &short).is_empty());
        let one_side_single = chart![step = 100; "x......", "...xx..", "x......", "...xx.."];
        assert!(detect(&Jumptrill, &one_side_single).is_empty());
    }
}
