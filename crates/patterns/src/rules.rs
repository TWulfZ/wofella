//! The rule registry. Rules live in `rules/<id>.rs`, one file each (architecture §9.2).

mod anchor;
mod bracket;
mod chordbracket;
mod chordjack;
mod chordstream_dense;
mod chordstream_light;
mod common;
mod handstream;
mod jumpstream;
mod jumptrill;
mod longjack;
mod minijack;
mod roll;
mod single;
mod split_trill;
mod trill;

pub use anchor::Anchor;
pub use bracket::Bracket;
pub use chordbracket::Chordbracket;
pub use chordjack::Chordjack;
pub use chordstream_dense::ChordstreamDense;
pub use chordstream_light::ChordstreamLight;
pub use handstream::Handstream;
pub use jumpstream::Jumpstream;
pub use jumptrill::Jumptrill;
pub use longjack::Longjack;
pub use minijack::Minijack;
pub use roll::Roll;
pub use single::Single;
pub use split_trill::SplitTrill;
pub use trill::Trill;

use crate::rule::PatternRule;

/// Fixed order: overlap resolution falls back to it after priority and strength, so appending
/// is safe and reordering changes outputs. Taxonomy order within each axis.
pub fn all() -> &'static [&'static dyn PatternRule] {
    &[
        &Minijack,
        &Chordjack,
        &Longjack,
        &Anchor,
        &Single,
        &Jumpstream,
        &Handstream,
        &ChordstreamLight,
        &ChordstreamDense,
        &Roll,
        &Trill,
        &Jumptrill,
        &SplitTrill,
        &Bracket,
        &Chordbracket,
    ]
}

#[cfg(test)]
pub(crate) mod testkit {
    use wolluf_chart::{
        Chart, ChartMeta, Diagnostics, Layout, Note, NoteKind, TimingKind, TimingPoint,
    };
    use wolluf_core::{ColMask, Keymode, PatternId, TimeUs};

    use crate::params::PatternParams;
    use crate::rule::{Candidate, PatternRule};
    use crate::view::ChartView;

    pub(crate) fn mask(cols: &[u8]) -> ColMask {
        ColMask::from_cols(Keymode::K7, cols.iter().copied()).unwrap()
    }

    pub(crate) fn ms(t: i32) -> TimeUs {
        TimeUs::from_ms(t)
    }

    pub(crate) fn cand(
        pattern: PatternId,
        t0: i32,
        t1: i32,
        cols: &[u8],
        strength: u32,
    ) -> Candidate {
        Candidate {
            pattern,
            t0: ms(t0),
            t1: ms(t1),
            cols: mask(cols),
            strength,
        }
    }

    pub(crate) fn detect(rule: &dyn PatternRule, chart: &Chart) -> Vec<Candidate> {
        detect_with(rule, chart, &Layout::default_for(chart.keymode()))
    }

    pub(crate) fn detect_with(
        rule: &dyn PatternRule,
        chart: &Chart,
        layout: &Layout,
    ) -> Vec<Candidate> {
        let params = PatternParams::default();
        let view = ChartView::new(chart, layout, &params).unwrap();
        rule.detect(&view, &params)
    }

    /// Same result under both 3|1+3 thumb sides: jack rules never read hands.
    pub(crate) fn detect_both_thumbs(rule: &dyn PatternRule, chart: &Chart) -> Vec<Candidate> {
        let right = detect_with(rule, chart, &Layout::by_id("k7.313_right_thumb").unwrap());
        let left = detect_with(rule, chart, &Layout::by_id("k7.313_left_thumb").unwrap());
        assert_eq!(right, left);
        right
    }

    /// Taps at `(ms, col)` in 7K, optionally under one red line at 0.
    pub(crate) fn taps(notes: &[(i32, u8)], beat_len_ms: Option<f64>) -> Chart {
        let notes = notes
            .iter()
            .map(|&(t, col)| Note {
                t: ms(t),
                col,
                kind: NoteKind::Tap,
            })
            .collect();
        Chart::from_notes(
            Keymode::K7,
            ChartMeta::default(),
            red(beat_len_ms),
            notes,
            &mut Diagnostics::new(),
        )
    }

    /// `chart` with one red line at 0.
    pub(crate) fn with_beat(chart: &Chart, beat_len_ms: f64) -> Chart {
        let taps = chart.rows().iter().flat_map(|r| {
            r.tap.iter().map(move |col| Note {
                t: r.t,
                col,
                kind: NoteKind::Tap,
            })
        });
        let holds = chart.ln_pairs().iter().map(|ln| Note {
            t: ln.head,
            col: ln.col,
            kind: NoteKind::Hold { end: ln.tail },
        });
        Chart::from_notes(
            chart.keymode(),
            ChartMeta::default(),
            red(Some(beat_len_ms)),
            taps.chain(holds).collect(),
            &mut Diagnostics::new(),
        )
    }

    fn red(beat_len_ms: Option<f64>) -> Vec<TimingPoint> {
        beat_len_ms
            .map(|beat_len_ms| TimingPoint {
                t: TimeUs::ZERO,
                kind: TimingKind::Uninherited {
                    beat_len_ms,
                    meter: 4,
                },
            })
            .into_iter()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn rule_ids_are_unique() {
        let ids: BTreeSet<String> = all().iter().map(|r| r.id().to_string()).collect();
        assert_eq!(ids.len(), all().len());
    }

    #[test]
    fn registry_order_is_fixed() {
        let ids: Vec<String> = all().iter().map(|r| r.id().to_string()).collect();
        assert_eq!(
            ids,
            [
                "regular.jack.minijack",
                "regular.jack.chordjack",
                "regular.jack.longjack",
                "regular.jack.anchor",
                "regular.stream.single",
                "regular.stream.jumpstream",
                "regular.stream.handstream",
                "regular.stream.chordstream_light",
                "regular.stream.chordstream_dense",
                "regular.stream.roll",
                "regular.stream.trill",
                "regular.stream.jumptrill",
                "regular.stream.split_trill",
                "regular.stream.bracket",
                "regular.stream.chordbracket",
            ]
        );
        assert!(all().iter().all(|r| r.version() >= 1));
    }
}

#[cfg(test)]
mod props {
    use proptest::prelude::*;
    use wolluf_chart::layout::preset_ids;
    use wolluf_chart::{
        Chart, ChartMeta, Diagnostics, Layout, Note, NoteKind, TimingKind, TimingPoint,
    };
    use wolluf_core::{ColMask, Keymode, TimeUs};

    use super::*;
    use crate::params::PatternParams;
    use crate::rule::{Candidate, STRENGTH_MAX};
    use crate::view::ChartView;

    const K: Keymode = Keymode::K7;

    fn arb_input() -> impl Strategy<Value = (Vec<Note>, Vec<TimingPoint>, usize)> {
        // Coarse times so jacks, chords and anchors actually occur.
        let note = (0i64..120, 0u8..7, prop::option::of(1i64..8)).prop_map(|(t, col, len)| {
            let t = TimeUs(t * 80_000);
            let kind = match len {
                None => NoteKind::Tap,
                Some(len) => NoteKind::Hold {
                    end: TimeUs(t.0 + len * 40_000),
                },
            };
            Note { t, col, kind }
        });
        let red = (0i32..5_000, 150u32..900).prop_map(|(t, beat)| TimingPoint {
            t: TimeUs::from_ms(t),
            kind: TimingKind::Uninherited {
                beat_len_ms: f64::from(beat),
                meter: 4,
            },
        });
        let scattered = prop::collection::vec(note, 0..200);
        // Alternations of disjoint masks and repeated shapes, which scattered notes almost
        // never form (jumptrills, dense chordstreams, brackets).
        let segment = (1u16..128, any::<u16>(), 2usize..9, any::<bool>());
        let shaped = prop::collection::vec(segment, 1..12).prop_map(|segments| {
            let mut notes = Vec::new();
            let mut row = 0i64;
            for (a, raw, len, alternate) in segments {
                let rest = !a & 0x7f;
                let b = if rest & raw == 0 { rest } else { rest & raw };
                for step in 0..len {
                    let mask = if alternate && step % 2 == 1 { b } else { a };
                    notes.extend((0u8..7).filter(|c| mask & (1 << c) != 0).map(|col| Note {
                        t: TimeUs(row * 80_000),
                        col,
                        kind: NoteKind::Tap,
                    }));
                    row += 1;
                }
            }
            notes
        });
        (
            prop_oneof![scattered, shaped],
            prop::collection::vec(red, 0..3),
            0usize..5,
        )
    }

    fn build(notes: Vec<Note>, timing: Vec<TimingPoint>) -> Chart {
        Chart::from_notes(
            K,
            ChartMeta::default(),
            timing,
            notes,
            &mut Diagnostics::new(),
        )
    }

    fn layout(preset: usize) -> Layout {
        Layout::by_id(preset_ids().nth(preset).unwrap()).unwrap()
    }

    fn mirrored(c: &Candidate) -> Candidate {
        Candidate {
            cols: c.cols.mirror(K),
            ..c.clone()
        }
    }

    proptest! {
        #[test]
        fn candidates_are_sorted_in_bounds_and_on_pressed_columns((notes, timing, preset) in arb_input()) {
            let chart = build(notes, timing);
            let layout = layout(preset);
            let params = PatternParams::default();
            let view = ChartView::new(&chart, &layout, &params).unwrap();
            for rule in all() {
                let found = rule.detect(&view, &params);
                prop_assert_eq!(&found, &rule.detect(&view, &params));
                prop_assert!(found.windows(2).all(|w| w[0] < w[1]), "{} not strictly sorted", rule.id());
                for c in &found {
                    prop_assert_eq!(&c.pattern, &rule.id());
                    prop_assert!(c.t0 <= c.t1);
                    prop_assert!(c.strength <= STRENGTH_MAX);
                    prop_assert!(!c.cols.is_empty());
                    let span: Vec<_> = view.rows().iter().filter(|r| c.t0 <= r.t && r.t <= c.t1).collect();
                    prop_assert!(span.first().is_some_and(|r| r.t == c.t0 && !r.press.is_empty()));
                    prop_assert!(span.last().is_some_and(|r| r.t == c.t1 && !r.press.is_empty()));
                    let pressed = span.iter().fold(0u16, |acc, r| acc | r.press.bits());
                    prop_assert!(c.cols.is_subset_of(ColMask::from_bits(K, pressed).unwrap()));
                }
            }
        }

        #[test]
        fn hand_agnostic_rules_are_mirror_symmetric((notes, timing, preset) in arb_input()) {
            let flipped: Vec<Note> = notes.iter().map(|n| Note { col: 6 - n.col, ..*n }).collect();
            let chart = build(notes, timing.clone());
            let mirror_chart = build(flipped, timing);
            let layout = layout(preset);
            let params = PatternParams::default();
            let a = ChartView::new(&chart, &layout, &params).unwrap();
            let b = ChartView::new(&mirror_chart, &layout, &params).unwrap();
            let agnostic: [&dyn PatternRule; 13] = [
                &Minijack, &Chordjack, &Longjack, &Anchor, &Single, &Jumpstream, &Handstream,
                &ChordstreamLight, &ChordstreamDense, &Roll, &Trill, &Jumptrill, &Chordbracket,
            ];
            for rule in agnostic {
                let mut expected: Vec<Candidate> = rule.detect(&a, &params).iter().map(mirrored).collect();
                expected.sort();
                prop_assert_eq!(rule.detect(&b, &params), expected, "{}", rule.id());
            }
        }

        #[test]
        fn every_rule_is_mirror_symmetric_with_a_mirrored_layout((notes, timing, preset) in arb_input()) {
            let flipped: Vec<Note> = notes.iter().map(|n| Note { col: 6 - n.col, ..*n }).collect();
            let chart = build(notes, timing.clone());
            let mirror_chart = build(flipped, timing);
            let layout = layout(preset);
            let mirror_layout = layout.mirror();
            let params = PatternParams::default();
            let a = ChartView::new(&chart, &layout, &params).unwrap();
            let b = ChartView::new(&mirror_chart, &mirror_layout, &params).unwrap();
            for rule in all() {
                let mut expected: Vec<Candidate> = rule.detect(&a, &params).iter().map(mirrored).collect();
                expected.sort();
                prop_assert_eq!(rule.detect(&b, &params), expected, "{}", rule.id());
            }
        }
    }
}
