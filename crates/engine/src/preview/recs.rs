//! Band recommendations over chart × rate MSD (ADR 0024, `preview.band_recs@1`): charts whose
//! focus skillset sits in a band around the player's preview rating. None of the thresholds is
//! calibrated.

use std::collections::{BTreeMap, BTreeSet};

use wolluf_core::ChartMd5;

use super::rating::PlayerRating;
use crate::stage::difficulty::SKILLSET_IDS;

/// Indices into `SKILLSET_IDS`.
pub const OVERALL: usize = 0;
pub const STAMINA: usize = 4;
pub const TECHNICAL: usize = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecsParams {
    /// Band edges around the target, centi MSD.
    pub band_lo_centi: i32,
    pub band_hi_centi: i32,
    /// Push aims above Overall so the list stretches the player instead of repeating them.
    pub push_target_centi: i32,
    /// The focus must be ≥ every other non-overall skillset minus this.
    pub dominance_margin_centi: i32,
    /// MinaCalc's technical runs high on most charts, so a focus only has to come within this
    /// of it.
    pub tech_dominance_margin_centi: i32,
    /// `SKILLSET_IDS` indices Deficit never picks for this keymode.
    pub excluded_focus: Vec<usize>,
    pub limit: usize,
    /// NM, HT and DT: playable without a rate copy.
    pub base_rates_milli: Vec<u16>,
    /// "Enable rates" adds these; a pick off `base_rates_milli` needs a rate copy.
    pub grid_rates_milli: Vec<u16>,
}

impl RecsParams {
    /// 7K technical and every stamina are excluded from Deficit: stamina is derived from the
    /// others, and MinaCalc's 7K technical is unvalidated (ADR 0024).
    pub fn for_keymode(keymode: u8) -> Self {
        let excluded_focus = if keymode == 7 {
            vec![STAMINA, TECHNICAL]
        } else {
            vec![STAMINA]
        };
        Self {
            band_lo_centi: -50,
            band_hi_centi: 150,
            push_target_centi: 50,
            dominance_margin_centi: 0,
            tech_dominance_margin_centi: 150,
            excluded_focus,
            limit: 30,
            base_rates_milli: vec![750, 1000, 1500],
            grid_rates_milli: (700..=1500).step_by(50).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The lowest rated skillset not in `excluded_focus`.
    Deficit,
    /// Overall, aimed `push_target_centi` above the player.
    Push,
    /// A `SKILLSET_IDS` index the player chose; 0 is Overall.
    Skillset(usize),
}

/// One chart at one mod rate; `centi` in `SKILLSET_IDS` order, Overall first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub md5: ChartMd5,
    /// One pick per beatmapset.
    pub set_key: String,
    pub rate_milli: u16,
    pub centi: [i32; 8],
    pub is_rate_copy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Deficit { skillset: usize, rating_centi: i32 },
    Push { target_centi: i32 },
    Skillset { skillset: usize },
    Unplayed,
    PlayedBefore,
    NeedsRateCopy { rate_milli: u16 },
    RateCopyInLibrary,
}

impl Reason {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Deficit { .. } => "deficit",
            Self::Push { .. } => "push",
            Self::Skillset { .. } => "skillset",
            Self::Unplayed => "unplayed",
            Self::PlayedBefore => "played_before",
            Self::NeedsRateCopy { .. } => "needs_rate_copy",
            Self::RateCopyInLibrary => "rate_copy_in_library",
        }
    }

    /// i18n arguments: skillsets as their `SKILLSET_IDS` id, numbers in centi or milli.
    pub fn args(&self) -> Vec<String> {
        match *self {
            Self::Deficit {
                skillset,
                rating_centi,
            } => vec![skillset_id(skillset), rating_centi.to_string()],
            Self::Push { target_centi } => vec![target_centi.to_string()],
            Self::Skillset { skillset } => vec![skillset_id(skillset)],
            Self::NeedsRateCopy { rate_milli } => vec![rate_milli.to_string()],
            Self::Unplayed | Self::PlayedBefore | Self::RateCopyInLibrary => Vec::new(),
        }
    }
}

fn skillset_id(i: usize) -> String {
    SKILLSET_IDS.get(i).copied().unwrap_or("unknown").to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecItem {
    pub md5: ChartMd5,
    pub set_key: String,
    pub rate_milli: u16,
    pub centi: [i32; 8],
    pub focus_centi: i32,
    /// `|focus_centi − target|`.
    pub distance_centi: i32,
    pub played: bool,
    pub is_rate_copy: bool,
    pub needs_rate_copy: bool,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecsOut {
    /// `SKILLSET_IDS` index; 0 for Push.
    pub focus: usize,
    /// Inclusive, absolute centi MSD.
    pub band: (i32, i32),
    pub items: Vec<RecItem>,
}

/// Picks are ordered unplayed first, then by distance to the target, md5 and rate. Within a
/// set the closest chart wins regardless of whether it was played.
pub fn recommend(
    rating: &PlayerRating,
    candidates: &[Candidate],
    played: &BTreeSet<ChartMd5>,
    mode: Mode,
    any_rate: bool,
    params: &RecsParams,
) -> RecsOut {
    let focus = match mode {
        Mode::Deficit => deficit_focus(rating, params),
        Mode::Push => OVERALL,
        Mode::Skillset(i) => i,
    };
    let Some(rating_centi) = rating_of(rating, focus) else {
        return RecsOut {
            focus,
            band: (0, 0),
            items: Vec::new(),
        };
    };
    let target = match mode {
        Mode::Push => rating_centi.saturating_add(params.push_target_centi),
        Mode::Deficit | Mode::Skillset(_) => rating_centi,
    };
    let band = (
        target.saturating_add(params.band_lo_centi),
        target.saturating_add(params.band_hi_centi),
    );

    let mut best: BTreeMap<&str, &Candidate> = BTreeMap::new();
    for c in candidates {
        let value = c.centi[focus];
        if value < band.0
            || value > band.1
            || !rate_allowed(c, any_rate, params)
            || !dominant(&c.centi, focus, params)
        {
            continue;
        }
        let slot = best.entry(c.set_key.as_str()).or_insert(c);
        if closeness(c, focus, target) < closeness(slot, focus, target) {
            *slot = c;
        }
    }

    let mut picks: Vec<&Candidate> = best.into_values().collect();
    picks.sort_by_cached_key(|c| (played.contains(&c.md5), closeness(c, focus, target)));
    picks.truncate(params.limit);

    let mode_reason = match mode {
        Mode::Deficit => Reason::Deficit {
            skillset: focus,
            rating_centi,
        },
        Mode::Push => Reason::Push {
            target_centi: target,
        },
        Mode::Skillset(skillset) => Reason::Skillset { skillset },
    };
    let items = picks
        .into_iter()
        .map(|c| {
            let was_played = played.contains(&c.md5);
            let needs_rate_copy =
                !c.is_rate_copy && !params.base_rates_milli.contains(&c.rate_milli);
            let mut reasons = vec![
                mode_reason,
                if was_played {
                    Reason::PlayedBefore
                } else {
                    Reason::Unplayed
                },
            ];
            if needs_rate_copy {
                reasons.push(Reason::NeedsRateCopy {
                    rate_milli: c.rate_milli,
                });
            } else if c.is_rate_copy {
                reasons.push(Reason::RateCopyInLibrary);
            }
            RecItem {
                md5: c.md5,
                set_key: c.set_key.clone(),
                rate_milli: c.rate_milli,
                centi: c.centi,
                focus_centi: c.centi[focus],
                distance_centi: distance(c.centi[focus], target),
                played: was_played,
                is_rate_copy: c.is_rate_copy,
                needs_rate_copy,
                reasons,
            }
        })
        .collect();
    RecsOut { focus, band, items }
}

/// The full key makes the order independent of the input order, even for duplicate candidates.
fn closeness(
    c: &Candidate,
    focus: usize,
    target: i32,
) -> (i32, ChartMd5, u16, &str, bool, [i32; 8]) {
    (
        distance(c.centi[focus], target),
        c.md5,
        c.rate_milli,
        c.set_key.as_str(),
        c.is_rate_copy,
        c.centi,
    )
}

fn distance(value: i32, target: i32) -> i32 {
    value.saturating_sub(target).saturating_abs()
}

fn rating_of(rating: &PlayerRating, focus: usize) -> Option<i32> {
    if focus == OVERALL {
        Some(rating.overall_centi)
    } else {
        rating.skillsets_centi.get(focus.checked_sub(1)?).copied()
    }
}

/// Ties keep the first skillset in `SKILLSET_IDS` order. Excluding every skillset falls back to
/// Overall rather than to an excluded one.
fn deficit_focus(rating: &PlayerRating, params: &RecsParams) -> usize {
    (1..SKILLSET_IDS.len())
        .filter(|i| !params.excluded_focus.contains(i))
        .min_by_key(|&i| (rating.skillsets_centi[i - 1], i))
        .unwrap_or(OVERALL)
}

/// A rate copy of a rate copy is never suggested: the original chart already covers its rates.
fn rate_allowed(c: &Candidate, any_rate: bool, params: &RecsParams) -> bool {
    if c.is_rate_copy {
        return c.rate_milli == 1000;
    }
    params.base_rates_milli.contains(&c.rate_milli)
        || (any_rate && params.grid_rates_milli.contains(&c.rate_milli))
}

fn dominant(centi: &[i32; 8], focus: usize, params: &RecsParams) -> bool {
    if focus == OVERALL {
        return true;
    }
    let value = centi[focus];
    (1..centi.len()).filter(|&i| i != focus).all(|i| {
        let margin = if i == TECHNICAL {
            params.tech_dominance_margin_centi
        } else {
            params.dominance_margin_centi
        };
        value >= centi[i].saturating_sub(margin)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STREAM: usize = 1;
    const JUMPSTREAM: usize = 2;
    const CHORDJACK: usize = 6;

    fn rating(overall: i32, skillsets: [i32; 7]) -> PlayerRating {
        PlayerRating {
            overall_centi: overall,
            skillsets_centi: skillsets,
            counted: Vec::new(),
        }
    }

    fn md5(n: u8) -> ChartMd5 {
        ChartMd5([n; 16])
    }

    /// `focus` at `value`, every other skillset well below it.
    fn cand(n: u8, set: &str, rate_milli: u16, focus: usize, value: i32) -> Candidate {
        let mut centi = [1000; 8];
        centi[focus] = value;
        Candidate {
            md5: md5(n),
            set_key: set.to_owned(),
            rate_milli,
            centi,
            is_rate_copy: false,
        }
    }

    fn k7() -> RecsParams {
        RecsParams::for_keymode(7)
    }

    fn k4() -> RecsParams {
        RecsParams::for_keymode(4)
    }

    fn none() -> BTreeSet<ChartMd5> {
        BTreeSet::new()
    }

    fn picks(out: &RecsOut) -> Vec<(u8, u16)> {
        out.items
            .iter()
            .map(|i| (i.md5.0[0], i.rate_milli))
            .collect()
    }

    fn codes(item: &RecItem) -> Vec<&'static str> {
        item.reasons.iter().map(Reason::code).collect()
    }

    #[test]
    fn params_match_the_frozen_contract() {
        let p = k7();
        assert_eq!((p.band_lo_centi, p.band_hi_centi), (-50, 150));
        assert_eq!(p.push_target_centi, 50);
        assert_eq!(
            (p.dominance_margin_centi, p.tech_dominance_margin_centi),
            (0, 150)
        );
        assert_eq!(p.limit, 30);
        assert_eq!(p.base_rates_milli, [750, 1000, 1500]);
        assert_eq!(p.grid_rates_milli.len(), 17);
        assert_eq!(p.grid_rates_milli.first(), Some(&700));
        assert_eq!(p.grid_rates_milli.last(), Some(&1500));
        assert!(p.grid_rates_milli.windows(2).all(|w| w[1] - w[0] == 50));
        assert_eq!(p.excluded_focus, [STAMINA, TECHNICAL]);
        assert_eq!(k4().excluded_focus, [STAMINA]);
        assert_eq!(SKILLSET_IDS[STAMINA], "stamina");
        assert_eq!(SKILLSET_IDS[TECHNICAL], "technical");
    }

    #[test]
    fn only_charts_inside_the_band_are_picked() {
        let r = rating(1800, [2000, 1500, 1500, 1500, 1500, 1500, 1500]);
        let c = [
            cand(1, "a", 1000, STREAM, 1949),
            cand(2, "b", 1000, STREAM, 1950),
            cand(3, "c", 1000, STREAM, 2150),
            cand(4, "d", 1000, STREAM, 2151),
        ];
        let out = recommend(&r, &c, &none(), Mode::Skillset(STREAM), false, &k7());
        assert_eq!(out.focus, STREAM);
        assert_eq!(out.band, (1950, 2150));
        assert_eq!(picks(&out), [(2, 1000), (3, 1000)]);
        assert_eq!(out.items[0].focus_centi, 1950);
        assert_eq!(out.items[0].distance_centi, 50);
        assert_eq!(
            out.items[0].reasons[0],
            Reason::Skillset { skillset: STREAM }
        );
        assert_eq!(out.items[0].reasons[0].args(), ["stream"]);
    }

    #[test]
    fn push_bands_overall_around_the_push_target() {
        let r = rating(2000, [1000; 7]);
        let mut c = [
            cand(1, "a", 1000, OVERALL, 1999),
            cand(2, "b", 1000, OVERALL, 2000),
            cand(3, "c", 1000, OVERALL, 2200),
            cand(4, "d", 1000, OVERALL, 2201),
        ];
        // Overall needs no dominance: a skillset far above the focus does not reject the chart.
        c[1].centi[JUMPSTREAM] = 3000;
        let out = recommend(&r, &c, &none(), Mode::Push, false, &k7());
        assert_eq!(out.focus, OVERALL);
        assert_eq!(out.band, (2000, 2200));
        assert_eq!(picks(&out), [(2, 1000), (3, 1000)]);
        assert_eq!(
            out.items
                .iter()
                .map(|i| i.distance_centi)
                .collect::<Vec<_>>(),
            [50, 150]
        );
        assert_eq!(out.items[0].reasons[0], Reason::Push { target_centi: 2050 });
        assert_eq!(out.items[0].reasons[0].code(), "push");
        assert_eq!(out.items[0].reasons[0].args(), ["2050"]);
    }

    #[test]
    fn deficit_focuses_the_lowest_skillset_not_excluded() {
        // stream, jumpstream, handstream, stamina, jackspeed, chordjack, technical
        let r = rating(2000, [2200, 2100, 2100, 1500, 2100, 1900, 1400]);
        let c = [
            cand(1, "a", 1000, CHORDJACK, 1900),
            cand(2, "b", 1000, TECHNICAL, 1400),
            cand(3, "c", 1000, STAMINA, 1500),
        ];
        let out7 = recommend(&r, &c, &none(), Mode::Deficit, false, &k7());
        assert_eq!(out7.focus, CHORDJACK);
        assert_eq!(out7.band, (1850, 2050));
        assert_eq!(picks(&out7), [(1, 1000)]);
        assert_eq!(
            out7.items[0].reasons[0],
            Reason::Deficit {
                skillset: CHORDJACK,
                rating_centi: 1900
            }
        );
        assert_eq!(out7.items[0].reasons[0].args(), ["chordjack", "1900"]);

        let out4 = recommend(&r, &c, &none(), Mode::Deficit, false, &k4());
        assert_eq!(out4.focus, TECHNICAL);
        assert_eq!(picks(&out4), [(2, 1000)]);
    }

    #[test]
    fn deficit_ties_take_the_first_skillset() {
        let r = rating(2000, [2100, 1900, 1900, 2100, 2100, 2100, 2100]);
        let out = recommend(&r, &[], &none(), Mode::Deficit, false, &k7());
        assert_eq!(out.focus, JUMPSTREAM);
    }

    #[test]
    fn the_focus_must_dominate_with_a_wider_margin_against_technical() {
        let r = rating(1800, [2000, 1500, 1500, 1500, 1500, 1500, 1500]);
        let with = |n: u8, other: usize, value: i32| {
            let mut c = cand(n, &format!("s{n}"), 1000, STREAM, 2000);
            c.centi[other] = value;
            c
        };
        let c = [
            with(1, JUMPSTREAM, 2001),
            with(2, JUMPSTREAM, 2000),
            with(3, TECHNICAL, 2150),
            with(4, TECHNICAL, 2151),
            with(5, OVERALL, 3000),
            with(6, STAMINA, 2001),
        ];
        let out = recommend(&r, &c, &none(), Mode::Skillset(STREAM), false, &k7());
        assert_eq!(picks(&out), [(2, 1000), (3, 1000), (5, 1000)]);

        // A technical focus gets no extra margin against the others.
        let mut tech = cand(7, "t", 1000, TECHNICAL, 1500);
        tech.centi[STREAM] = 1501;
        let out = recommend(
            &rating(1800, [1500; 7]),
            &[tech],
            &none(),
            Mode::Skillset(TECHNICAL),
            false,
            &k7(),
        );
        assert!(out.items.is_empty());
    }

    #[test]
    fn one_pick_per_set_the_closest_then_by_md5_then_rate() {
        let r = rating(1800, [2000, 1500, 1500, 1500, 1500, 1500, 1500]);
        let c = [
            cand(9, "a", 1000, STREAM, 2000),
            cand(3, "b", 1000, STREAM, 2030),
            cand(2, "b", 1000, STREAM, 2010),
            cand(1, "b", 1000, STREAM, 1990),
            cand(5, "c", 1500, STREAM, 2050),
            cand(5, "c", 750, STREAM, 2050),
        ];
        let out = recommend(&r, &c, &none(), Mode::Skillset(STREAM), false, &k7());
        assert_eq!(picks(&out), [(9, 1000), (1, 1000), (5, 750)]);
    }

    #[test]
    fn unplayed_charts_come_first() {
        let r = rating(1800, [2000, 1500, 1500, 1500, 1500, 1500, 1500]);
        let c = [
            cand(1, "a", 1000, STREAM, 2000),
            cand(2, "b", 1000, STREAM, 2100),
            cand(3, "c", 1000, STREAM, 2050),
        ];
        let played = BTreeSet::from([md5(1)]);
        let out = recommend(&r, &c, &played, Mode::Skillset(STREAM), false, &k7());
        assert_eq!(picks(&out), [(3, 1000), (2, 1000), (1, 1000)]);
        assert_eq!(codes(&out.items[0]), ["skillset", "unplayed"]);
        assert!(!out.items[0].played);
        assert_eq!(codes(&out.items[2]), ["skillset", "played_before"]);
        assert!(out.items[2].played);
    }

    fn rate_candidates() -> Vec<Candidate> {
        let mut c: Vec<Candidate> = [700, 750, 1000, 1100, 1234, 1500]
            .into_iter()
            .enumerate()
            .map(|(i, rate)| {
                let n = i as u8 + 1;
                cand(n, &format!("s{n}"), rate, STREAM, 2000)
            })
            .collect();
        for (n, rate) in [(20, 1000), (21, 1500)] {
            let mut copy = cand(n, &format!("s{n}"), rate, STREAM, 2000);
            copy.is_rate_copy = true;
            c.push(copy);
        }
        c
    }

    #[test]
    fn without_any_rate_only_base_rates_and_rate_copies_at_1000() {
        let r = rating(1800, [2000, 1500, 1500, 1500, 1500, 1500, 1500]);
        let out = recommend(
            &r,
            &rate_candidates(),
            &none(),
            Mode::Skillset(STREAM),
            false,
            &k7(),
        );
        assert_eq!(picks(&out), [(2, 750), (3, 1000), (6, 1500), (20, 1000)]);
        assert!(out.items.iter().all(|i| !i.needs_rate_copy));
        let copy = &out.items[3];
        assert!(copy.is_rate_copy);
        assert_eq!(
            codes(copy),
            ["skillset", "unplayed", "rate_copy_in_library"]
        );
        assert_eq!(codes(&out.items[0]), ["skillset", "unplayed"]);
    }

    #[test]
    fn any_rate_adds_grid_rates_that_need_a_rate_copy() {
        let r = rating(1800, [2000, 1500, 1500, 1500, 1500, 1500, 1500]);
        let out = recommend(
            &r,
            &rate_candidates(),
            &none(),
            Mode::Skillset(STREAM),
            true,
            &k7(),
        );
        assert_eq!(
            picks(&out),
            [
                (1, 700),
                (2, 750),
                (3, 1000),
                (4, 1100),
                (6, 1500),
                (20, 1000)
            ]
        );
        let needs: Vec<bool> = out.items.iter().map(|i| i.needs_rate_copy).collect();
        assert_eq!(needs, [true, false, false, true, false, false]);
        assert_eq!(
            codes(&out.items[3]),
            ["skillset", "unplayed", "needs_rate_copy"]
        );
        assert_eq!(
            out.items[3].reasons[2],
            Reason::NeedsRateCopy { rate_milli: 1100 }
        );
        assert_eq!(out.items[3].reasons[2].args(), ["1100"]);
    }

    #[test]
    fn output_does_not_depend_on_candidate_order() {
        let r = rating(1800, [2000, 1500, 1500, 1500, 1500, 1500, 1500]);
        let mut c = rate_candidates();
        c.extend([
            cand(30, "a", 1000, STREAM, 2040),
            cand(31, "a", 1000, STREAM, 1960),
            cand(32, "b", 1000, STREAM, 2100),
        ]);
        let played = BTreeSet::from([md5(3), md5(32)]);
        let base = recommend(&r, &c, &played, Mode::Skillset(STREAM), true, &k7());
        assert!(!base.items.is_empty());
        for shift in 1..c.len() {
            let mut rotated = c.clone();
            rotated.rotate_left(shift);
            assert_eq!(
                recommend(&r, &rotated, &played, Mode::Skillset(STREAM), true, &k7()),
                base
            );
        }
        c.reverse();
        assert_eq!(
            recommend(&r, &c, &played, Mode::Skillset(STREAM), true, &k7()),
            base
        );
    }

    #[test]
    fn the_list_stops_at_the_limit() {
        let r = rating(1800, [2000, 1500, 1500, 1500, 1500, 1500, 1500]);
        let c: Vec<Candidate> = (1..=5u8)
            .map(|n| cand(n, &format!("s{n}"), 1000, STREAM, 2000 + i32::from(n)))
            .collect();
        let params = RecsParams { limit: 3, ..k7() };
        let out = recommend(&r, &c, &none(), Mode::Skillset(STREAM), false, &params);
        assert_eq!(picks(&out), [(1, 1000), (2, 1000), (3, 1000)]);
    }

    #[test]
    fn a_chosen_skillset_ignores_the_deficit_exclusions() {
        let r = rating(1800, [1500; 7]);
        let c = [cand(1, "a", 1000, TECHNICAL, 1500)];
        let out = recommend(&r, &c, &none(), Mode::Skillset(TECHNICAL), false, &k7());
        assert_eq!(out.focus, TECHNICAL);
        assert_eq!(picks(&out), [(1, 1000)]);
    }

    #[test]
    fn an_unknown_skillset_recommends_nothing() {
        let r = rating(1800, [1500; 7]);
        let c = [cand(1, "a", 1000, STREAM, 1500)];
        let out = recommend(&r, &c, &none(), Mode::Skillset(8), false, &k7());
        assert_eq!(out.focus, 8);
        assert!(out.items.is_empty());
    }
}
