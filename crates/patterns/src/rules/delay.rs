//! `regular.speed.delay`: the BMS ディレイ shape, filed under speed by the Jinjin 7K dans
//! (ADR 0017 amendment). Notes and small chords staggered off the 1/4 grid: a press row is a
//! delay row when its press gap is at most `delay_max_gap_us` whatever the grid says, or, under
//! a red line and within one beat, snaps to a divisor of at least `delay_min_divisor` or to
//! none while inside half a beat. Read at map tempo, so rate never changes it. Windows of
//! `delay_window_rows` press rows whose delay share reaches `delay_min_share_permille` merge
//! into candidates. Seeded from mania-hub `offGridRowShare` (MIT, see NOTICE).

use wolluf_core::{Keymode, PatternId};

use super::common::{permille, rows_of, span_candidate, window_spans};
use crate::params::{PatternParams, SpeedParams, TICKS_PER_BEAT};
use crate::rule::{Candidate, PatternRule};
use crate::view::{ChartView, RowFeat, ticks};

pub(super) const ID: PatternId = PatternId::from_static("regular.speed.delay");

pub struct Delay;

impl PatternRule for Delay {
    fn id(&self) -> PatternId {
        ID
    }

    fn version(&self) -> u32 {
        1
    }

    fn supports(&self, _keymode: Keymode) -> bool {
        true
    }

    /// Strength: the delay-row share over the whole span.
    fn detect(&self, view: &ChartView<'_>, params: &PatternParams) -> Vec<Candidate> {
        let p = &params.speed;
        let rows = view.rows();
        let share = |span: &[usize]| delay_share(&rows_of(rows, span), p);
        let mut found: Vec<Candidate> =
            window_spans(rows, p.delay_window_rows, p.delay_window_max_gap_us, |w| {
                share(w) >= p.delay_min_share_permille
            })
            .iter()
            .filter_map(|span| {
                span_candidate(ID, view.keymode(), &rows_of(rows, span), share(span))
            })
            .collect();
        found.sort();
        found
    }
}

fn is_delay(row: &RowFeat, p: &SpeedParams) -> bool {
    let Some(gap) = row.press_gap_us else {
        return false;
    };
    if gap <= p.delay_max_gap_us {
        return true;
    }
    let Some(beat) = row.beat_us.filter(|&b| gap <= b) else {
        return false;
    };
    match row.snap {
        Some(d) => d >= p.delay_min_divisor,
        None => ticks(gap, beat) <= TICKS_PER_BEAT / 2,
    }
}

fn delay_share(span: &[&RowFeat], p: &SpeedParams) -> u32 {
    let delayed = span.iter().filter(|r| is_delay(r, p)).count();
    permille(
        u64::try_from(delayed).unwrap_or(0),
        u64::try_from(span.len()).unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::testkit::{cand, detect, seq};

    const ALL: [u8; 7] = [0, 1, 2, 3, 4, 5, 6];

    fn alternating(a: i32, b: i32, n: usize) -> Vec<i32> {
        (1..n).map(|k| if k % 2 == 1 { a } else { b }).collect()
    }

    #[test]
    fn staggered_sixteenth_offsets_are_delay() {
        // 150 BPM: a 1/4 grid whose rows are split 1/16 + 3/16 beat apart.
        let chart = seq(&alternating(25, 75, 20), &ALL, Some(400.0));
        assert_eq!(detect(&Delay, &chart), [cand(ID, 0, 925, &ALL, 950)]);
    }

    #[test]
    fn sextuplet_flow_is_delay() {
        // 160 BPM 1/6: 62.5 ms rounded to whole stable milliseconds.
        let chart = seq(&alternating(62, 63, 20), &ALL, Some(375.0));
        assert_eq!(detect(&Delay, &chart), [cand(ID, 0, 1187, &ALL, 950)]);
    }

    #[test]
    fn gaps_under_the_floor_are_delay_without_a_red_line() {
        let chart = seq(&[50; 19], &ALL, None);
        assert_eq!(detect(&Delay, &chart), [cand(ID, 0, 950, &ALL, 950)]);
    }

    #[test]
    fn quarter_streams_and_untimed_charts_are_not_delay() {
        assert!(detect(&Delay, &seq(&[100; 19], &ALL, Some(400.0))).is_empty());
        assert!(detect(&Delay, &seq(&[75; 19], &ALL, None)).is_empty());
    }

    #[test]
    fn near_misses() {
        // Seven 1/8 gaps in a 1/4 stream: at most 7 of 16 rows per window.
        let mut gaps = vec![100; 19];
        for k in [1, 3, 5, 7, 9, 11, 13] {
            gaps[k] = 50;
        }
        assert!(detect(&Delay, &seq(&gaps, &ALL, Some(400.0))).is_empty());
        let short = seq(&alternating(25, 75, 15), &ALL, Some(400.0));
        assert!(detect(&Delay, &short).is_empty());
    }
}
