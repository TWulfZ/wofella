//! A play's goal for MinaCalc's SSR (ADR 0024). stable stores no Wife%, so each observed
//! judgement scores the mean Wife3 J4 points over its timing-error interval and a miss scores
//! Wife3's miss weight.
//!
//! ScoreV2 judges LN heads and tails apart and its header mixes both (research 03 l.121-127). A
//! tail is judged on `|release| / 1.5` with late releases cut at the 100 edge, but the counts do
//! not say which judgements are tails, so every judgement takes the note-window value. LN-heavy
//! charts are excluded upstream, so tails stay a minority of a counted play.

use super::mods::PlayMods;
use super::params::GoalParams;
use super::play::PlayCounts;

/// stable ScoreV1 note windows before mods (research 03 l.121-127): MAX is fixed, the others
/// fall by `3·OD`.
const MAX_WINDOW_MS: f64 = 16.0;
/// ScoreV2 MAX (`rejudge.py:windows`): `22.4 - 0.6·OD` up to OD 5, `24.9 - 1.1·OD` above; the two
/// meet at OD 5. The other V2 windows are V1's.
const V2_MAX_KNEE_OD: f64 = 5.0;
const V2_MAX_LOW: (f64, f64) = (22.4, 0.6);
const V2_MAX_HIGH: (f64, f64) = (24.9, 1.1);
const WINDOW_BASE_MS: [f64; 4] = [64.0, 97.0, 127.0, 151.0];
const WINDOW_OD_SLOPE: f64 = 3.0;
/// HR divides and EZ multiplies the windows by 1.4 (research 03).
const HR_EZ_FACTOR: f64 = 1.4;
/// `rejudge.py:_fl`: `64 - 3·OD` style products land a hair under an integer in binary floats.
const FLOOR_EPS_MS: f64 = 1e-6;

/// `RageUtil.h:95` and `:126`.
const WIFE3_MISS_WEIGHT: f64 = -5.5;
const WIFE3_MAX_POINTS: f64 = 2.0;

const PERMYRIAD: f64 = 10_000.0;
const MILLI: f64 = 1_000.0;

/// `None` when the play has no judgement at all.
pub fn goal_permyriad(
    counts: PlayCounts,
    od: f32,
    mods: PlayMods,
    params: &GoalParams,
) -> Option<u16> {
    let total = counts.total();
    if total == 0 {
        return None;
    }
    let hits = [
        counts.max,
        counts.n300,
        counts.n200,
        counts.n100,
        counts.n50,
    ]
    .map(u32::from);
    let values = bucket_points(&windows(od, mods, params), params);
    let points = f64::from(counts.miss) * WIFE3_MISS_WEIGHT
        + hits
            .iter()
            .zip(values)
            .map(|(&n, v)| f64::from(n) * v)
            .sum::<f64>();
    let wife = points / (WIFE3_MAX_POINTS * f64::from(total));
    let wife = wife.min(params.cap).clamp(0.0, 1.0);
    Some((wife * PERMYRIAD).round() as u16)
}

/// Real-time judgement edges in ms. stable keeps real-time windows fixed under rate mods, but
/// judges whole map-time ms against `floor(base × rate)` (research 03), so the floor happens in
/// map time before the rate is divided back out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Windows {
    /// `|e|` edges of MAX, 300, 200, 100 and 50 on the early side; MAX, 300 and 200 are symmetric.
    pub early: [f64; 5],
    /// stable judges a late press up to `100 - 1` ms as a 100 and has no late 50
    /// (`rejudge.py:note_judge`, research 03 l.121-127).
    pub late_100: f64,
}

/// `(lo, hi, sides)`: `|e|` in `[lo, hi)` on `sides` sides of the note.
type Piece = (f64, f64, f64);

impl Windows {
    /// Each judgement's `|e|` pieces, MAX first; an unused second piece is empty.
    pub(crate) fn buckets(&self) -> [[Piece; 2]; 5] {
        let [m, g, gd, o, b] = self.early;
        let late = self.late_100.max(gd).min(o);
        [
            [(0.0, m, 2.0), EMPTY],
            [(m, g, 2.0), EMPTY],
            [(g, gd, 2.0), EMPTY],
            [(gd, late, 2.0), (late, o, 1.0)],
            [(o, b, 1.0), EMPTY],
        ]
    }
}

const EMPTY: Piece = (0.0, 0.0, 0.0);

pub(crate) fn windows(od: f32, mods: PlayMods, params: &GoalParams) -> Windows {
    let od = f64::from(od);
    let factor = if mods.hr {
        1.0 / HR_EZ_FACTOR
    } else if mods.ez {
        HR_EZ_FACTOR
    } else {
        1.0
    };
    let rate = f64::from(mods.rate_milli()) / MILLI;
    let max = if !mods.score_v2 {
        MAX_WINDOW_MS
    } else if od <= V2_MAX_KNEE_OD {
        V2_MAX_LOW.0 - V2_MAX_LOW.1 * od
    } else {
        V2_MAX_HIGH.0 - V2_MAX_HIGH.1 * od
    };
    let base = [
        max,
        WINDOW_BASE_MS[0] - WINDOW_OD_SLOPE * od,
        WINDOW_BASE_MS[1] - WINDOW_OD_SLOPE * od,
        WINDOW_BASE_MS[2] - WINDOW_OD_SLOPE * od,
        WINDOW_BASE_MS[3] - WINDOW_OD_SLOPE * od,
    ];
    let map_ms = base.map(|b| (b * factor * rate + FLOOR_EPS_MS).floor());
    let real = |ms: f64| (ms + params.edge_pad_ms) / rate;
    Windows {
        early: map_ms.map(real),
        late_100: real(map_ms[3] - LATE_EDGE_MS),
    }
}

const LATE_EDGE_MS: f64 = 1.0;

/// Mean Wife3 points of a MAX, 300, 200, 100 and 50 under a uniform error over each interval.
/// A Gaussian σ fitted to the counts couples every judgement's value to the others: a miss
/// turned into a 50 widens σ and can lower the goal (1850 MAX and 18 misses, EZ OD 0: 96.39%
/// to 96.32%), so the values stay independent of the play.
pub(crate) fn bucket_points(windows: &Windows, params: &GoalParams) -> [f64; 5] {
    windows
        .buckets()
        .map(|pieces| mean_points(&pieces, |_| 1.0, params))
}

/// Mean Wife3 points over `pieces` weighted by `density(|e|)`. Simpson runs per smooth piece of
/// the curve; its kinks would otherwise cost the rule its order.
pub(crate) fn mean_points(
    pieces: &[Piece],
    density: impl Fn(f64) -> f64,
    params: &GoalParams,
) -> f64 {
    let ts = params.timing_scale;
    let kinks = [
        f64::from(WIFE3_RIDIC_MS * ts),
        f64::from(WIFE3_ZERO_MS * libm::powf(ts, WIFE3_J_POW)),
        f64::from(WIFE3_MAX_BOO_MS * ts),
    ];
    let n = usize::from(params.simpson_intervals.max(2) & !1);
    let points = |x: f64| f64::from(wife3((x / MILLI) as f32, ts));
    let (mut weighted, mut mass) = (0.0, 0.0);
    for &(lo, hi, sides) in pieces.iter().filter(|p| p.1 > p.0) {
        let mut cuts = vec![lo];
        cuts.extend(kinks.into_iter().filter(|&k| k > lo && k < hi));
        cuts.push(hi);
        for w in cuts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let h = (b - a) / n as f64;
            for i in 0..=n {
                let x = a + h * i as f64;
                let simpson = if i == 0 || i == n {
                    1.0
                } else if i % 2 == 1 {
                    4.0
                } else {
                    2.0
                };
                let f = sides * simpson * h * density(x);
                weighted += f * points(x);
                mass += f;
            }
        }
    }
    if mass > 0.0 {
        weighted / mass
    } else {
        points((pieces[0].0 + pieces[0].1) / 2.0)
    }
}

const WIFE3_RIDIC_MS: f32 = 5.0;
const WIFE3_ZERO_MS: f32 = 65.0;
const WIFE3_DEV_MS: f32 = 22.7;
const WIFE3_MAX_BOO_MS: f32 = 180.0;
const WIFE3_J_POW: f32 = 0.75;

/// Etterna `wife3` (`RageUtil.h:121-155`), offset in seconds, in `float` as upstream.
pub(crate) fn wife3(offset_s: f32, ts: f32) -> f32 {
    const MAX_POINTS: f32 = 2.0;
    const MISS_WEIGHT: f32 = -5.5;
    let ridic = WIFE3_RIDIC_MS * ts;
    let max_boo_weight = WIFE3_MAX_BOO_MS * ts;
    let ms = (offset_s * 1000.0).abs();
    if ms <= ridic {
        return MAX_POINTS;
    }
    let zero = WIFE3_ZERO_MS * libm::powf(ts, WIFE3_J_POW);
    let dev = WIFE3_DEV_MS * libm::powf(ts, WIFE3_J_POW);
    if ms <= zero {
        return MAX_POINTS * erf_as((zero - ms) / dev);
    }
    if ms <= max_boo_weight {
        return (ms - zero) * MISS_WEIGHT / (max_boo_weight - zero);
    }
    MISS_WEIGHT
}

const A1: f32 = 0.254_829_6;
const A2: f32 = -0.284_496_72;
const A3: f32 = 1.421_413_8;
const A4: f32 = -1.453_152_1;
const A5: f32 = 1.061_405_4;
const P: f32 = 0.327_591_1;

/// Upstream's own erf (Abramowitz & Stegun 7.1.26, `RageUtil.h:97-119`), kept instead of
/// `libm::erff` because Wife3 points are defined by it.
fn erf_as(x: f32) -> f32 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + P * x);
    let y = 1.0 - (((((A5 * t + A4) * t) + A3) * t + A2) * t + A1) * t * libm::expf(-x * x);
    sign * y
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const HR: i32 = 16;
    const EZ: i32 = 2;
    const DT: i32 = 64;
    const NC: i32 = 512;
    const HT: i32 = 256;
    const V2: i32 = 1 << 29;

    fn c(max: u16, n300: u16, n200: u16, n100: u16, n50: u16, miss: u16) -> PlayCounts {
        PlayCounts {
            max,
            n300,
            n200,
            n100,
            n50,
            miss,
        }
    }

    fn goal(counts: PlayCounts, od: f32, bits: i32) -> Option<u16> {
        goal_permyriad(
            counts,
            od,
            PlayMods::from_bits(bits),
            &GoalParams::default(),
        )
    }

    /// scipy `quad` of the half-normal mean of upstream Wife3 cut at the OD 8 50 edge, halved.
    #[test]
    fn half_normal_points_match_numeric_integration() {
        let p = GoalParams::default();
        for (sigma, want) in [
            (20.0, 0.977_650_756),
            (27.6, 0.919_454_503),
            (36.3, 0.810_292_066),
            (50.0, 0.612_517_805),
        ] {
            let got = mean_points(&[(0.0, 127.5, 1.0)], gaussian(sigma), &p) / WIFE3_MAX_POINTS;
            assert!((got - want).abs() < 2e-5, "σ {sigma}: {got} vs {want}");
        }
    }

    /// `-0.284496736F` in `RageUtil.h` rounds to this float.
    #[test]
    fn erf_constants_match_upstream_floats() {
        assert_eq!(A2.to_bits(), 0xbe91_a98e);
    }

    const MOD_BITS: [i32; 8] = [0, HR, EZ, DT, HT, V2, V2 | HR, V2 | DT];

    fn arb_count() -> impl Strategy<Value = u16> {
        prop_oneof![1 => Just(0u16), 3 => 0..3000u16]
    }

    proptest! {
        #[test]
        fn every_one_step_improvement_never_lowers_the_goal(
            counts in prop::array::uniform6(arb_count()),
            od in 0.0f32..=10.0,
            m in 0..MOD_BITS.len(),
        ) {
            let bits = MOD_BITS[m];
            let Some(g0) = goal(from(counts), od, bits) else {
                return Ok(());
            };
            for k in 1..6 {
                if counts[k] == 0 {
                    continue;
                }
                let mut better = counts;
                better[k] -= 1;
                better[k - 1] += 1;
                let g1 = goal(from(better), od, bits).unwrap();
                prop_assert!(g1 >= g0, "{counts:?} od {od} mods {bits} tier {k}: {g1} < {g0}");
            }
        }
    }

    /// g++ 13 -O0 x86-64 build of upstream `RageUtil.h:93-155` with `ts = 1`.
    #[test]
    fn wife3_matches_upstream() {
        let cases: [(f32, u32); 13] = [
            (0.0, 0x4000_0000),
            (0.004, 0x4000_0000),
            (0.005, 0x4000_0000),
            (0.0051, 0x3fff_f389),
            (0.012, 0x3fff_c10f),
            (0.03, 0x3ff8_850b),
            (0.0449, 0x3fca_1da6),
            (0.065, 0x0000_0000),
            (0.08, 0xbf37_a6f5),
            (0.1225, 0xc030_0000),
            (0.18, 0xc0b0_0000),
            (0.25, 0xc0b0_0000),
            (-0.03, 0x3ff8_850b),
        ];
        for (x, bits) in cases {
            let want = f32::from_bits(bits);
            let got = wife3(x, 1.0);
            assert!(
                (got - want).abs() <= 1e-6,
                "wife3({x}) = {got}, upstream {want}"
            );
        }
    }

    #[test]
    fn windows_follow_stable_v1_in_real_time() {
        let p = GoalParams::default();
        let pad = p.edge_pad_ms;
        let nm = windows(8.0, PlayMods::from_bits(0), &p);
        assert_eq!(
            nm.early,
            [16.0 + pad, 40.0 + pad, 73.0 + pad, 103.0 + pad, 127.0 + pad]
        );
        // OD 8.3: 64 - 24.9 = 39.1 floors to 39.
        let frac = windows(8.3, PlayMods::from_bits(0), &p);
        assert_eq!(frac.early[1], 39.0 + pad);
        let hr = windows(8.0, PlayMods::from_bits(HR), &p);
        assert_eq!(
            hr.early,
            [11.0 + pad, 28.0 + pad, 52.0 + pad, 73.0 + pad, 90.0 + pad]
        );
        let ez = windows(8.0, PlayMods::from_bits(EZ), &p);
        assert_eq!(
            ez.early,
            [
                22.0 + pad,
                56.0 + pad,
                102.0 + pad,
                144.0 + pad,
                177.0 + pad
            ]
        );
        // Map-time windows are floor(base × rate); real time divides the rate back out.
        let dt = windows(8.0, PlayMods::from_bits(DT), &p);
        assert_eq!(dt.early[0], (24.0 + pad) / 1.5);
        assert_eq!(dt.early[1], (60.0 + pad) / 1.5);
        assert_eq!(dt.early[3], (154.0 + pad) / 1.5);
        let ht = windows(8.0, PlayMods::from_bits(HT), &p);
        assert_eq!(ht.early[0], (12.0 + pad) / 0.75);
        assert_eq!(ht.early[2], (54.0 + pad) / 0.75);
    }

    /// `rejudge.py:windows`: the V2 MAX window falls with OD; the other windows are V1's.
    #[test]
    fn windows_follow_stable_v2_max_by_od() {
        let p = GoalParams::default();
        let pad = p.edge_pad_ms;
        let v2 = |od: f32, bits: i32| windows(od, PlayMods::from_bits(V2 | bits), &p);
        for (od, max) in [
            (0.0, 22.0),
            (5.0, 19.0),
            (8.0, 16.0),
            (9.0, 15.0),
            (10.0, 13.0),
        ] {
            assert_eq!(v2(od, 0).early[0], max + pad, "od {od}");
        }
        // OD 3: 22.4 - 1.8 = 20.6 floors to 20.
        assert_eq!(v2(3.0, 0).early[0], 20.0 + pad);
        let v1 = windows(8.0, PlayMods::from_bits(0), &p);
        assert_eq!(v2(8.0, 0).early[1..], v1.early[1..]);
        assert_eq!(v2(8.0, 0).late_100, v1.late_100);
        // 16.1 / 1.4 = 11.5 and 16.1 × 1.5 = 24.15 in map time.
        assert_eq!(v2(8.0, HR).early[0], 11.0 + pad);
        assert_eq!(v2(8.0, DT).early[0], (24.0 + pad) / 1.5);
    }

    #[test]
    fn score_v2_counts_judge_against_the_v2_max_window() {
        let mid = from(BASES[0]);
        // Same 16 ms MAX at OD 8.
        assert_eq!(goal(mid, 8.0, V2), goal(mid, 8.0, 0));
        // A wider MAX at OD 0 is worth less per MAX, a narrower one at OD 10 more.
        assert!(goal(mid, 0.0, V2).unwrap() < goal(mid, 0.0, 0).unwrap());
        assert!(goal(mid, 10.0, V2).unwrap() > goal(mid, 10.0, 0).unwrap());
    }

    /// `rejudge.py:note_judge`: `-O <= d <= O - 1` is a 100, `-M <= d < -O` a 50, `d > O - 1` a
    /// miss, in whole map-time ms.
    #[test]
    fn late_hits_stop_one_ms_early_and_have_no_50() {
        let p = GoalParams::default();
        let pad = p.edge_pad_ms;
        assert_eq!(
            windows(8.0, PlayMods::from_bits(0), &p).late_100,
            102.0 + pad
        );
        assert_eq!(
            windows(8.0, PlayMods::from_bits(HR), &p).late_100,
            72.0 + pad
        );
        assert_eq!(
            windows(8.0, PlayMods::from_bits(DT), &p).late_100,
            (153.0 + pad) / 1.5
        );
        let b = windows(8.0, PlayMods::from_bits(0), &p).buckets();
        assert_eq!(b[3], [(73.5, 102.5, 2.0), (102.5, 103.5, 1.0)]);
        assert_eq!(b[4][0], (103.5, 127.5, 1.0));
        assert!(b[4][1].1 <= b[4][1].0, "no late 50");
    }

    fn gaussian(sigma: f64) -> impl Fn(f64) -> f64 {
        move |x| libm::exp(-x * x / (2.0 * sigma * sigma))
    }

    /// scipy `quad` of upstream Wife3 over the OD 8 nomod intervals, uniform density.
    #[test]
    fn bucket_points_match_numeric_integration() {
        let p = GoalParams::default();
        let got = bucket_points(&windows(8.0, PlayMods::from_bits(0), &p), &p);
        let want = [
            1.998_708_182,
            1.927_136_394,
            0.709_985_892,
            -1.112_159_175,
            -2.415_217_391,
        ];
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 4e-5, "{got:?} vs {want:?}");
        }
    }

    #[test]
    fn bucket_points_fall_from_max_to_50_under_every_window() {
        let p = GoalParams::default();
        for bits in MOD_BITS {
            for od in [0.0f32, 3.7, 8.0, 10.0] {
                let v = bucket_points(&windows(od, PlayMods::from_bits(bits), &p), &p);
                assert!(
                    v.windows(2).all(|w| w[0] > w[1]) && v[4] > WIFE3_MISS_WEIGHT,
                    "od {od} mods {bits}: {v:?}"
                );
            }
        }
    }

    /// The counterexample that ruled out a fitted Gaussian σ.
    #[test]
    fn a_miss_turned_50_never_lowers_a_tight_ez_play() {
        let before = goal(c(1850, 0, 0, 0, 0, 18), 0.0, EZ).unwrap();
        let after = goal(c(1850, 0, 0, 0, 1, 17), 0.0, EZ).unwrap();
        assert!(after >= before, "{after} < {before}");
    }

    #[test]
    fn expected_points_fall_with_sigma_and_stay_in_range() {
        let p = GoalParams::default();
        let cut = [(0.0, 127.5, 1.0)];
        let tight = mean_points(&cut, gaussian(0.5), &p);
        assert!((tight - 2.0).abs() < 1e-9, "{tight}");
        let mut prev = tight;
        for sigma in [5.0, 10.0, 20.0, 40.0, 80.0] {
            let e = mean_points(&cut, gaussian(sigma), &p);
            assert!(e < prev && e > -5.5, "σ {sigma}: {e}");
            prev = e;
        }
    }

    #[test]
    fn no_judgements_have_no_goal() {
        assert_eq!(goal(PlayCounts::default(), 8.0, 0), None);
    }

    #[test]
    fn all_max_reaches_the_cap() {
        assert_eq!(goal(c(1000, 0, 0, 0, 0, 0), 8.0, 0), Some(9650));
        assert_eq!(goal(c(1, 0, 0, 0, 0, 0), 0.0, HR), Some(9650));
    }

    #[test]
    fn all_miss_is_zero() {
        assert_eq!(goal(c(0, 0, 0, 0, 0, 50), 8.0, 0), Some(0));
    }

    /// Below the cap, where a change can show.
    const BASES: [[u16; 6]; 5] = [
        [500, 400, 150, 60, 20, 15],
        [300, 300, 200, 100, 50, 40],
        [200, 300, 150, 50, 10, 3],
        [50, 40, 20, 10, 5, 2],
        [1000, 600, 100, 30, 10, 10],
    ];

    fn from(a: [u16; 6]) -> PlayCounts {
        c(a[0], a[1], a[2], a[3], a[4], a[5])
    }

    #[test]
    fn more_max_never_lowers_and_more_miss_always_lowers() {
        for base in BASES {
            let g0 = goal(from(base), 8.0, 0).unwrap();
            let mut more_max = base;
            more_max[0] += 50;
            let mut more_miss = base;
            more_miss[5] += 5;
            assert!(goal(from(more_max), 8.0, 0).unwrap() > g0, "{base:?}");
            assert!(goal(from(more_miss), 8.0, 0).unwrap() < g0, "{base:?}");
        }
    }

    #[test]
    fn a_better_judgement_never_lowers_the_goal() {
        for base in BASES {
            for od in [0.0f32, 5.0, 8.0, 9.5] {
                let g0 = goal(from(base), od, 0).unwrap();
                for k in 1..6 {
                    if base[k] == 0 {
                        continue;
                    }
                    let mut better = base;
                    better[k] -= 1;
                    better[k - 1] += 1;
                    let g1 = goal(from(better), od, 0).unwrap();
                    assert!(g1 >= g0, "{base:?} od {od} tier {k}: {g1} < {g0}");
                }
            }
        }
    }

    #[test]
    fn stricter_windows_mean_a_tighter_player() {
        let counts = from(BASES[0]);
        let nm = goal(counts, 8.0, 0).unwrap();
        assert!(goal(counts, 8.0, HR).unwrap() > nm);
        assert!(goal(counts, 8.0, EZ).unwrap() < nm);
        assert!(goal(counts, 9.0, 0).unwrap() > nm);
        // Real-time windows barely move with rate.
        let dt = goal(counts, 8.0, DT | NC).unwrap();
        assert!(dt.abs_diff(nm) < 50, "{dt} vs {nm}");
    }

    /// The scipy bucket values of `bucket_points_match_numeric_integration` give 0.939 560 and
    /// 0.733 263.
    #[test]
    fn reference_plays_match_the_numeric_model() {
        assert_eq!(goal(c(1500, 400, 50, 15, 5, 10), 8.0, 0), Some(9396));
        assert_eq!(goal(from(BASES[0]), 8.0, 0), Some(7333));
        assert_eq!(goal(c(950, 50, 0, 0, 0, 0), 8.0, 0), Some(9650));
    }
}
