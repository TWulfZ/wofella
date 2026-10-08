//! `play_ssr`: a play's goal and its MinaCalc SSRs (ADR 0024). The key is stage-level plus the
//! goal and exclusion params hashes (sections `preview.goal`, `preview.exclusion`: a row's status
//! depends on the exclusion rules) and a config hash of the keymode's `difficulty` key, which
//! already carries the calc version; plays are memoized per id in the cache table, so no per-play
//! input enters it.

use wolluf_core::{StageId, VersionKey, VersionKeyBuilder};

use super::difficulty::{Calc, CalcError, Chart};
use super::hash_field;
use crate::error::EngineError;
use crate::preview::{ExclusionParams, GoalParams};

pub const STAGE: StageId = StageId::from_static("play_ssr");
pub const VERSION: u32 = 2;

const CONFIG_TAG: &[u8] = b"wolluf.play_ssr.config.v1";

/// `difficulty_vkey` is the key of the MSD rows of the play's keymode.
pub fn vkey(
    difficulty_vkey: VersionKey,
    goal: &GoalParams,
    exclusion: &ExclusionParams,
) -> Result<VersionKey, EngineError> {
    Ok(VersionKeyBuilder::new(STAGE, VERSION)
        .section("preview.goal", goal.params_hash())
        .section("preview.exclusion", exclusion.params_hash())
        .config(config_hash(difficulty_vkey))
        .finish()?)
}

/// A play's SSRs in `SKILLSET_IDS` order, centi. `goal` is Wife% (1.0 = 100%); `rate_milli` is the
/// mod rate. `calc` keeps native scratch buffers, so a caller holds one per worker thread.
pub fn run(
    calc: &mut Calc,
    chart: &Chart,
    rate_milli: u16,
    goal: f32,
) -> Result<[i32; 8], CalcError> {
    let rows = wolluf_difficulty::minacalc::note_rows(chart).rows;
    wolluf_difficulty::minacalc::ssr_centi(calc, &rows, rate_milli, goal, chart.keymode().columns())
}

/// The `difficulty` key in the ADR 0006 encoding: a new calc, rate grid or parse re-rates plays.
fn config_hash(difficulty_vkey: VersionKey) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CONFIG_TAG);
    hash_field(&mut hasher, &difficulty_vkey.0);
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use wolluf_core::Keymode;

    use super::*;
    use crate::stage::{chart_parse, difficulty};

    #[test]
    fn run_rates_a_stream_and_rejects_a_chart_without_notes() {
        let mut calc = Calc::new().unwrap();
        let stream = wolluf_chart::chart![step = 110;
            "x...", ".x..", "..x.", "...x", "..x.", ".x..", "x...", ".x..",
            "..x.", "...x", "..x.", ".x..", "x...", ".x..", "..x.", "...x",
        ];
        let ssr = run(&mut calc, &stream, 1000, 0.93).unwrap();
        assert!(ssr[0] > 0, "{ssr:?}");
        let faster = run(&mut calc, &stream, 1500, 0.93).unwrap();
        assert!(faster[0] > ssr[0], "{faster:?} vs {ssr:?}");

        let silent = wolluf_chart::chart![step = 110; "....", "...."];
        assert_eq!(run(&mut calc, &silent, 1000, 0.93), Err(CalcError::Empty));
    }

    #[test]
    fn stage_id_and_version_are_stable() {
        assert_eq!(STAGE.as_str(), "play_ssr");
        // 2: ScoreV2 plays are counted on V2 windows instead of excluded.
        assert_eq!(VERSION, 2);
    }

    #[test]
    fn vkey_depends_on_the_difficulty_key_and_goal_params() {
        let k4 = difficulty::vkey(
            chart_parse::vkey().unwrap(),
            Keymode::K4,
            &difficulty::MinaCalcParams::default(),
        )
        .unwrap();
        let params = GoalParams::default();
        let key = vkey(k4, &params, &ExclusionParams::default()).unwrap();
        assert_eq!(key, vkey(k4, &params, &ExclusionParams::default()).unwrap());
        assert_ne!(
            key,
            vkey(VersionKey([7; 32]), &params, &ExclusionParams::default()).unwrap(),
            "difficulty"
        );
        let capped = GoalParams {
            cap: 0.96,
            ..params.clone()
        };
        let ex = ExclusionParams::default();
        assert_ne!(key, vkey(k4, &capped, &ex).unwrap(), "goal params");
        assert_ne!(key, k4);
        // Stored statuses depend on which mods are excluded.
        for changed in [
            ExclusionParams {
                exclude_random: false,
                ..ex
            },
            ExclusionParams {
                exclude_coop: false,
                ..ex
            },
            ExclusionParams {
                exclude_key_mod: false,
                ..ex
            },
        ] {
            assert_ne!(key, vkey(k4, &params, &changed).unwrap(), "{changed:?}");
        }
    }

    #[test]
    fn vkey_is_frozen() {
        let upstream = chart_parse::vkey().unwrap();
        let mc = difficulty::MinaCalcParams::default();
        let params = GoalParams::default();
        let ex = ExclusionParams::default();
        // Frozen: a change re-keys every stored play SSR.
        let k4 = difficulty::vkey(upstream, Keymode::K4, &mc).unwrap();
        let k7 = difficulty::vkey(upstream, Keymode::K7, &mc).unwrap();
        assert_eq!(
            vkey(k4, &params, &ex).unwrap().to_string(),
            "23f2295998b49ff01d9b9e2529061c217e50fd15bfac08880421c2b954e418fb"
        );
        assert_eq!(
            vkey(k7, &params, &ex).unwrap().to_string(),
            "c65ae3527a1f1d28f9e616bdef5bbd44cd296f314f82911ef5fc192cd23f4dca"
        );
    }
}
