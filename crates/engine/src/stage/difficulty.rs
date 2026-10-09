//! `difficulty`: MinaCalc skillsets of a parsed chart across the rate grid (ADR 0022, ADR 0023).
//! The key is stage-level plus the params hash (section `minacalc.k<N>`) and a config hash of
//! the upstream `chart_parse` key, the calc version and the keymode; charts are memoized per md5
//! in `derivation.input_key`, so no per-chart input enters it.

use wolluf_core::{Keymode, StageId, VersionKey, VersionKeyBuilder};

use super::hash_field;
use crate::error::EngineError;

/// Re-exported so callers above the engine (app may not depend on difficulty, D1) can name the
/// stage's calculator, params and output.
pub use wolluf_chart::Chart;
pub use wolluf_difficulty::minacalc::{
    MinaCalcParams, MsdAtRate, MsdStatus, MsdTable, UnratedReason,
};
pub use wolluf_difficulty::{CALC_VERSION, Calc, CalcError, SKILLSET_IDS};

pub const STAGE: StageId = StageId::from_static("difficulty");
pub const VERSION: u32 = 2;

const CONFIG_TAG: &[u8] = b"wolluf.difficulty.config.v1";

/// `calc` keeps native scratch buffers, so a caller holds one per worker thread.
pub fn run(calc: &mut Calc, chart: &Chart, params: &MinaCalcParams) -> MsdTable {
    wolluf_difficulty::minacalc::msd_table(calc, chart, params)
}

/// The stage-level key of `keymode`'s MSD rows; `chart_parse_vkey` is the key of the parsed
/// rows they were computed from.
pub fn vkey(
    chart_parse_vkey: VersionKey,
    keymode: Keymode,
    params: &MinaCalcParams,
) -> Result<VersionKey, EngineError> {
    vkey_with(chart_parse_vkey, keymode, params, CALC_VERSION)
}

fn vkey_with(
    upstream: VersionKey,
    keymode: Keymode,
    params: &MinaCalcParams,
    calc_version: i32,
) -> Result<VersionKey, EngineError> {
    Ok(VersionKeyBuilder::new(STAGE, VERSION)
        .section(
            format!("minacalc.k{}", keymode.columns()),
            params.params_hash(),
        )
        .config(config_hash(upstream, keymode, calc_version))
        .finish()?)
}

/// Fields in the ADR 0006 encoding: the `chart_parse` key (a new parse re-rates), the calc as
/// `minacalc@<version>` (ADR 0022: a new vendored calc recomputes, and the stage golden never
/// sees calculator output), and the keymode (MinaCalc runs per-keycount logic).
fn config_hash(upstream: VersionKey, keymode: Keymode, calc_version: i32) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CONFIG_TAG);
    hash_field(&mut hasher, &upstream.0);
    hash_field(&mut hasher, format!("minacalc@{calc_version}").as_bytes());
    hasher.update(&u32::from(keymode.columns()).to_le_bytes());
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use wolluf_chart::chart;
    use wolluf_core::Keymode;

    use super::*;
    use crate::stage::chart_parse;

    fn rice_k4() -> Chart {
        chart![step = 120; "x...", ".x..", "..xx", "x..x", ".xx.", "...x", "xx..", "..x."]
    }

    fn ln_heavy_k7() -> Chart {
        chart![step = 100;
            "[[[....",
            "|||x...",
            "]]]....",
            "...[[[.",
            "x..|||.",
            "...]]]x",
        ]
    }

    #[test]
    fn stage_id_and_version_are_stable() {
        assert_eq!(STAGE.as_str(), "difficulty");
        assert_eq!(VERSION, 2);
    }

    #[test]
    fn vkey_depends_on_upstream_keymode_params_and_calc_version() {
        let upstream = chart_parse::vkey().unwrap();
        let params = MinaCalcParams::default();
        let key = vkey(upstream, Keymode::K7, &params).unwrap();
        assert_eq!(key, vkey(upstream, Keymode::K7, &params).unwrap());
        assert_ne!(
            key,
            vkey(upstream, Keymode::K4, &params).unwrap(),
            "keymode"
        );
        assert_ne!(
            key,
            vkey(VersionKey([9; 32]), Keymode::K7, &params).unwrap(),
            "another chart_parse key"
        );
        let mut coarser = params.clone();
        coarser.rate_grid_milli.retain(|r| r % 100 == 0);
        assert_ne!(
            key,
            vkey(upstream, Keymode::K7, &coarser).unwrap(),
            "rate grid"
        );
        let mut ln = params.clone();
        ln.ln_unrated_hold_share_permille += 1;
        assert_ne!(key, vkey(upstream, Keymode::K7, &ln).unwrap(), "LN cut-off");
        assert_eq!(
            key,
            vkey_with(upstream, Keymode::K7, &params, CALC_VERSION).unwrap()
        );
        assert_ne!(
            key,
            vkey_with(upstream, Keymode::K7, &params, CALC_VERSION + 1).unwrap(),
            "a new vendored calc"
        );
    }

    #[test]
    fn vkey_is_frozen() {
        let upstream = chart_parse::vkey().unwrap();
        let params = MinaCalcParams::default();
        // Frozen: a change re-keys every stored MSD row.
        assert_eq!(
            vkey(upstream, Keymode::K7, &params).unwrap().to_string(),
            "42def7ca6a8ee5fbf2666f7a65f1de932fd581cf071b18b3f5d2a56fedf12673"
        );
        assert_eq!(
            vkey(upstream, Keymode::K4, &params).unwrap().to_string(),
            "fa30c8b5bace09f0876175f52c4a87b19beeb0423e438e0bbcaeb928d819c7ac"
        );
    }

    #[test]
    fn run_rates_rice_on_every_grid_rate_and_skips_ln_heavy_charts() {
        let mut calc = Calc::new().unwrap();
        let params = MinaCalcParams::default();
        let rice = run(&mut calc, &rice_k4(), &params);
        assert_eq!(rice.status, MsdStatus::Rated);
        let rates: Vec<u16> = rice.rows.iter().map(|r| r.rate_milli).collect();
        assert_eq!(rates, params.rate_grid_milli);
        assert!(rice.rows.iter().all(|r| r.centi[0] > 0), "{rice:?}");

        let ln = run(&mut calc, &ln_heavy_k7(), &params);
        assert_eq!(ln.status, MsdStatus::Unrated(UnratedReason::LnHeavy));
        assert!(ln.hold_share_permille >= params.ln_unrated_hold_share_permille);
        assert!(ln.rows.is_empty());
    }
}
