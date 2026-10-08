//! Etterna's player rating over per-play SSRs (ADR 0024), ported from etterna
//! `2d706147cc4bcf95a8ceb24dc4f4129be207f803` (MIT): `ScoreManager.cpp` `SetTopScores`,
//! `CalcPlayerRating`, `SortTopSSRPtrs` and `MinaCalcHelpers.h` `aggregate_skill`.

use super::params::{AggregateParams, RatingParams};

/// One play's SSRs in `SKILLSET_IDS` order (index 0 is Overall), centi as the store keeps them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RatedPlay<'a> {
    pub family: &'a str,
    /// Mod rate × the chart's own rate.
    pub rate_milli: u16,
    pub ssr_centi: [i32; 8],
    /// Breaks Overall ties: the earlier play keeps the PB, as Etterna keeps a PB until beaten.
    pub played_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRating {
    pub overall_centi: i32,
    /// The 7 non-overall skillsets, in `SKILLSET_IDS[1..]` order.
    pub skillsets_centi: [i32; 7],
    /// Indices into the input of the plays that count, best Overall first.
    pub counted: Vec<usize>,
}

const CENTI: f32 = 100.0;

pub fn aggregate_rating(ssrs: &[f32]) -> f32 {
    aggregate_with(ssrs, &AggregateParams::ETTERNA)
}

/// `aggregate_skill` (`MinaCalcHelpers.h:34-66`): a search for the rating whose `2^(r/10)`
/// equals the summed erfc weights. Upstream mixes `float` and `double`; the port keeps each
/// operation's type so results match bit for bit.
pub fn aggregate_with(ssrs: &[f32], params: &AggregateParams) -> f32 {
    let mut rating = params.start_rating;
    let mut resolution = params.resolution;
    for _ in 0..params.iterations {
        loop {
            rating += resolution;
            let mut sum = 0.0f64;
            for &v in ssrs {
                let x = params.delta_multiplier * f64::from(v - rating);
                sum += (2.0 / libm::erfc(x) - 2.0).max(0.0);
            }
            // Upstream loops `while (pow < sum)`, so a NaN sum stops the search instead of spinning.
            let below = libm::pow(2.0, f64::from(rating) * 0.1) < sum;
            if !below {
                break;
            }
        }
        rating -= resolution;
        resolution /= 2.0;
    }
    rating += resolution * 2.0;
    rating * params.result_multiplier
}

/// Etterna keeps one PB per chart and rate (`ScoresForChart::ScoresByRate`); a family stands in
/// for the chart so rate copies compete. Best Overall wins, ties keep the earliest played, then
/// the earliest in the input.
pub fn rate_pbs(plays: &[RatedPlay<'_>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..plays.len()).collect();
    order.sort_by(|&a, &b| {
        let (pa, pb) = (&plays[a], &plays[b]);
        (pa.family, pa.rate_milli)
            .cmp(&(pb.family, pb.rate_milli))
            .then(pb.ssr_centi[0].cmp(&pa.ssr_centi[0]))
            .then(earlier(plays, a, b))
    });
    let mut pbs: Vec<usize> = Vec::new();
    let mut last: Option<(&str, u16)> = None;
    for i in order {
        let key = (plays[i].family, plays[i].rate_milli);
        if last != Some(key) {
            pbs.push(i);
            last = Some(key);
        }
    }
    pbs.sort_unstable();
    pbs
}

/// `SetTopScores` (`ScoreManager.cpp:269-320`): the best `top_per_family` rate PBs of a family by
/// Overall. Output is best Overall first, ties earliest played first.
pub fn top2_per_family(
    plays: &[RatedPlay<'_>],
    candidates: &[usize],
    params: &RatingParams,
) -> Vec<usize> {
    let mut by_family = candidates.to_vec();
    by_family.sort_by(|&a, &b| {
        plays[a]
            .family
            .cmp(plays[b].family)
            .then(plays[b].ssr_centi[0].cmp(&plays[a].ssr_centi[0]))
            .then(earlier(plays, a, b))
    });
    let mut top: Vec<usize> = Vec::new();
    let mut family: Option<&str> = None;
    let mut taken = 0u8;
    for i in by_family {
        if family != Some(plays[i].family) {
            family = Some(plays[i].family);
            taken = 0;
        }
        if taken < params.top_per_family {
            top.push(i);
            taken += 1;
        }
    }
    top.sort_by(|&a, &b| {
        plays[b].ssr_centi[0]
            .cmp(&plays[a].ssr_centi[0])
            .then(earlier(plays, a, b))
    });
    top
}

fn earlier(plays: &[RatedPlay<'_>], a: usize, b: usize) -> core::cmp::Ordering {
    plays[a]
        .played_at_ms
        .cmp(&plays[b].played_at_ms)
        .then(a.cmp(&b))
}

/// `CalcPlayerRating` (`ScoreManager.cpp:874-900`): each non-overall skillset aggregates the
/// counted plays' SSRs in ascending order (`SortTopSSRPtrs`, `:937`), clamped; Overall is their
/// mean, not an aggregate (`:897`).
pub fn player_rating(plays: &[RatedPlay<'_>], params: &RatingParams) -> Option<PlayerRating> {
    if plays.is_empty() {
        return None;
    }
    let counted = top2_per_family(plays, &rate_pbs(plays), params);
    let mut skillsets = [0.0f32; 7];
    for (k, slot) in skillsets.iter_mut().enumerate() {
        let mut ssrs: Vec<f32> = counted
            .iter()
            .map(|&i| plays[i].ssr_centi[k + 1] as f32 / CENTI)
            .collect();
        ssrs.sort_by(f32::total_cmp);
        *slot =
            aggregate_with(&ssrs, &params.aggregate).clamp(params.min_rating, params.max_rating);
    }
    let overall = skillsets.iter().sum::<f32>() / skillsets.len() as f32;
    Some(PlayerRating {
        overall_centi: to_centi(overall),
        skillsets_centi: skillsets.map(to_centi),
        counted,
    })
}

fn to_centi(x: f32) -> i32 {
    (x * CENTI).round() as i32
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    /// `aggregate_skill(v, 0.1L, 1.05f, 0.0, 10.24f)` compiled with g++ 13 (-O0, x86-64) from
    /// upstream `MinaCalcHelpers.h:34-66`, inputs sorted ascending as `SortTopSSRPtrs` does.
    #[test]
    fn aggregate_matches_upstream_bit_for_bit() {
        let ramp: Vec<f32> = (0..40u8)
            .map(|i| 15.0f32 + 0.37f32 * f32::from(i))
            .collect();
        let cases: [(&str, Vec<f32>, u32); 10] = [
            ("empty", vec![], 0x3c2c_0830),
            ("one0", vec![0.0], 0x3c2c_0830),
            ("one30", vec![30.0], 0x41bd_d70a),
            ("two30", vec![30.0, 30.0], 0x41cc_9fbe),
            ("30_25", vec![25.0, 30.0], 0x41c0_9cac),
            ("30_25_20", vec![20.0, 25.0, 30.0], 0x41c0_9cac),
            ("30_29_28_27", vec![27.0, 28.0, 29.0, 30.0], 0x41d0_a7ef),
            ("fifty25", vec![25.0; 50], 0x41ce_22d1),
            ("ramp40", ramp, 0x41d8_f8d4),
            ("one100", vec![100.0], 0x42aa_1ef9),
        ];
        for (name, v, bits) in cases {
            let got = aggregate_rating(&v);
            assert_eq!(
                got.to_bits(),
                bits,
                "{name}: got {got}, upstream {}",
                f32::from_bits(bits)
            );
        }
    }

    fn play(family: &str, rate_milli: u16, overall: i32) -> RatedPlay<'_> {
        RatedPlay {
            family,
            rate_milli,
            ssr_centi: [overall; 8],
            played_at_ms: 0,
        }
    }

    fn centi(x: f32) -> i32 {
        (x * 100.0).round() as i32
    }

    #[test]
    fn no_plays_have_no_rating() {
        assert_eq!(player_rating(&[], &RatingParams::default()), None);
    }

    #[test]
    fn a_single_play_rates_as_its_upstream_aggregate() {
        let r = player_rating(&[play("a", 1000, 3000)], &RatingParams::default()).unwrap();
        // Etterna rates a lone 30.00 SSR at 23.73, not 30.
        assert_eq!(r.overall_centi, 2373);
        assert_eq!(r.skillsets_centi, [2373; 7]);
        assert_eq!(r.counted, vec![0]);
    }

    #[test]
    fn one_pb_per_family_and_rate() {
        let plays = [
            play("a", 1000, 2800),
            play("a", 1000, 3000),
            play("a", 1000, 2900),
            play("b", 1000, 2500),
        ];
        assert_eq!(rate_pbs(&plays), vec![1, 3]);
        let r = player_rating(&plays, &RatingParams::default()).unwrap();
        assert_eq!(r.counted, vec![1, 3]);
        assert_eq!(r.overall_centi, centi(aggregate_rating(&[25.0, 30.0])));
    }

    #[test]
    fn ties_keep_the_earliest_played() {
        let mut later = play("a", 1000, 3000);
        later.played_at_ms = 200;
        let mut earlier = play("a", 1000, 3000);
        earlier.played_at_ms = 100;
        assert_eq!(rate_pbs(&[later, earlier]), vec![1]);
        assert_eq!(rate_pbs(&[earlier, later]), vec![0]);
        // Same instant: input order decides, so the result stays deterministic.
        assert_eq!(
            rate_pbs(&[play("a", 1000, 3000), play("a", 1000, 3000)]),
            vec![0]
        );
        let mut b = play("b", 1000, 3000);
        b.played_at_ms = 50;
        assert_eq!(
            top2_per_family(
                &[later, earlier, b],
                &[0, 1, 2],
                &RatingParams {
                    top_per_family: 1,
                    ..RatingParams::default()
                }
            ),
            vec![2, 1]
        );
    }

    #[test]
    fn only_the_two_best_rates_of_a_family_count() {
        let plays = [
            play("a", 1000, 2500),
            play("a", 1100, 2700),
            play("a", 1200, 2900),
            play("b", 900, 2000),
        ];
        let pbs = rate_pbs(&plays);
        assert_eq!(pbs, vec![0, 1, 2, 3]);
        assert_eq!(
            top2_per_family(&plays, &pbs, &RatingParams::default()),
            vec![2, 1, 3]
        );
        let one = RatingParams {
            top_per_family: 1,
            ..RatingParams::default()
        };
        assert_eq!(top2_per_family(&plays, &pbs, &one), vec![2, 3]);
        let r = player_rating(&plays, &RatingParams::default()).unwrap();
        assert_eq!(r.counted, vec![2, 1, 3]);
    }

    #[test]
    fn skillsets_aggregate_the_plays_chosen_by_overall() {
        let mut stream_heavy = play("a", 1000, 2600);
        stream_heavy.ssr_centi[1] = 4000;
        let mut best = play("a", 1100, 3000);
        best.ssr_centi[1] = 2000;
        best.ssr_centi[7] = 3500;
        let plays = [stream_heavy, best, play("a", 1200, 2800)];
        let r = player_rating(&plays, &RatingParams::default()).unwrap();
        assert_eq!(r.counted, vec![1, 2]);
        // The stream-heavy play is not a top-2 rate of its chart, so its 40.00 stream is unused.
        let stream = aggregate_rating(&[20.0, 28.0]);
        let tech = aggregate_rating(&[28.0, 35.0]);
        let rest = aggregate_rating(&[28.0, 30.0]);
        assert_eq!(r.skillsets_centi[0], centi(stream));
        assert_eq!(r.skillsets_centi[6], centi(tech));
        assert_eq!(r.skillsets_centi[1..6], [centi(rest); 5]);
        let mean = (stream + rest + rest + rest + rest + rest + tech) / 7.0;
        assert_eq!(r.overall_centi, centi(mean));
    }

    #[test]
    fn ratings_clamp_to_100() {
        let r = player_rating(&[play("a", 1000, 20_000)], &RatingParams::default()).unwrap();
        assert_eq!(r.skillsets_centi, [10_000; 7]);
        assert_eq!(r.overall_centi, 10_000);
    }

    fn arb_plays() -> impl Strategy<Value = Vec<(u8, u8, [i32; 8])>> {
        prop::collection::vec((0..6u8, 0..4u8, prop::array::uniform8(0..4000i32)), 1..24)
    }

    const FAMILIES: [&str; 7] = ["a", "b", "c", "d", "e", "f", "new"];

    fn to_plays(raw: &[(u8, u8, [i32; 8])]) -> Vec<RatedPlay<'static>> {
        raw.iter()
            .map(|(f, r, ssr)| RatedPlay {
                family: FAMILIES[usize::from(*f)],
                rate_milli: 900 + 100 * u16::from(*r),
                ssr_centi: *ssr,
                played_at_ms: 0,
            })
            .collect()
    }

    proptest! {
        #[test]
        fn adding_a_lower_play_never_lowers_a_rating(raw in arb_plays(), low in 0..1000i32) {
            let params = RatingParams::default();
            let mut plays = to_plays(&raw);
            let before = player_rating(&plays, &params).unwrap();
            plays.push(RatedPlay { family: "new", rate_milli: 1000, ssr_centi: [low; 8], played_at_ms: 0 });
            let after = player_rating(&plays, &params).unwrap();
            prop_assert!(after.overall_centi >= before.overall_centi);
            for (a, b) in after.skillsets_centi.iter().zip(before.skillsets_centi) {
                prop_assert!(*a >= b);
            }
        }

        #[test]
        fn two_plays_of_one_family_and_rate_never_both_count(raw in arb_plays()) {
            let plays = to_plays(&raw);
            let r = player_rating(&plays, &RatingParams::default()).unwrap();
            let mut keys: Vec<(&str, u16)> =
                r.counted.iter().map(|&i| (plays[i].family, plays[i].rate_milli)).collect();
            let n = keys.len();
            keys.sort_unstable();
            keys.dedup();
            prop_assert_eq!(keys.len(), n);
            for f in FAMILIES {
                prop_assert!(r.counted.iter().filter(|&&i| plays[i].family == f).count() <= 2);
            }
        }
    }
}
