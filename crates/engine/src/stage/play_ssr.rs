//! `play_ssr`: a play's goal and its MinaCalc SSRs (ADR 0024). The key is stage-level plus the
//! goal and exclusion params hashes (sections `preview.goal`, `preview.exclusion`: a row's status
//! depends on the exclusion rules) and a config hash of the keymode's `difficulty` key, which
//! already carries the calc version; plays are memoized per id in the cache table, so no per-play
//! input enters it.

use wolluf_core::{StageId, VersionKey, VersionKeyBuilder};

use super::hash_field;
use crate::error::EngineError;
use crate::preview::{ExclusionParams, GoalParams};

pub const STAGE: StageId = StageId::from_static("play_ssr");
pub const VERSION: u32 = 1;

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
    fn stage_id_and_version_are_stable() {
        assert_eq!(STAGE.as_str(), "play_ssr");
        assert_eq!(VERSION, 1);
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
            "856ecc9ea2d33672ab49a114349f871db097af2b0703bd96d0f4ea8e924e8f7b"
        );
        assert_eq!(
            vkey(k7, &params, &ex).unwrap().to_string(),
            "e6bd1fe2b52227375571e6fd7c93339b50da3b16e5a3524426649f9ce8d2f5c4"
        );
    }
}
