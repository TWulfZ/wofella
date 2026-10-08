//! Preview thresholds and constants (D17). None of them is calibrated (ADR 0024).

use serde::Serialize;

use super::dan::DanTable4k;

const GOAL_PARAMS_TAG: &[u8] = b"wolluf.preview.goal.params.v1";
const EXCLUSION_PARAMS_TAG: &[u8] = b"wolluf.preview.exclusion.params.v1";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PreviewParams {
    pub goal: GoalParams,
    pub rating: RatingParams,
    pub evidence: EvidenceParams,
    pub exclusion: ExclusionParams,
    pub family: FamilyParams,
    pub dan_k4: DanTable4k,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GoalParams {
    /// Etterna's `ssr_goal_cap` (`MinaCalcHelpers.h:11`): SSRs stop growing past 96.5%.
    pub cap: f64,
    /// Wife3 judge scale; 1.0 is J4.
    pub timing_scale: f32,
    /// Replay offsets are whole map-time ms, so a judgement at `|d| <= w` covers the continuous
    /// error `|e| < w + pad`.
    pub edge_pad_ms: f64,
    /// Simpson intervals per smooth piece of the Wife3 curve (even).
    pub simpson_intervals: u16,
}

impl Default for GoalParams {
    fn default() -> Self {
        Self {
            cap: 0.965,
            timing_scale: 1.0,
            edge_pad_ms: 0.5,
            simpson_intervals: 128,
        }
    }
}

impl GoalParams {
    pub fn params_hash(&self) -> [u8; 32] {
        params_hash(GOAL_PARAMS_TAG, self)
    }
}

/// blake3 over a domain tag and the postcard encoding (field order; floats as IEEE bytes).
fn params_hash(tag: &[u8], params: &impl Serialize) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(tag);
    // Derived `Serialize` over numbers and bools cannot fail with the allocating flavour; the
    // error arm still hashes to a distinct value instead of panicking.
    match postcard::to_allocvec(params) {
        Ok(bytes) => hasher.update(&bytes),
        Err(err) => hasher.update(err.to_string().as_bytes()),
    };
    *hasher.finalize().as_bytes()
}

/// Etterna `aggregate_skill(v, 0.1L, 1.05, 0.0, 10.24)` (`ScoreManager.cpp:889`), with the
/// iteration count of `MinaCalcHelpers.h:43`. Types follow upstream: the sum is `double`, the
/// rating `float`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AggregateParams {
    pub delta_multiplier: f64,
    pub result_multiplier: f32,
    pub start_rating: f32,
    pub resolution: f32,
    pub iterations: u8,
}

impl AggregateParams {
    pub const ETTERNA: Self = Self {
        delta_multiplier: 0.1,
        result_multiplier: 1.05,
        start_rating: 0.0,
        resolution: 10.24,
        iterations: 11,
    };
}

impl Default for AggregateParams {
    fn default() -> Self {
        Self::ETTERNA
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatingParams {
    pub aggregate: AggregateParams,
    /// Etterna's `SetTopScores` marks the two best rate PBs of a chart (`ScoreManager.cpp:301-317`).
    pub top_per_family: u8,
    /// `CLAMP(pskillsets[ss], 0.F, 100.F)` (`ScoreManager.cpp:890`).
    pub min_rating: f32,
    pub max_rating: f32,
}

impl Default for RatingParams {
    fn default() -> Self {
        Self {
            aggregate: AggregateParams::ETTERNA,
            top_per_family: 2,
            min_rating: 0.0,
            max_rating: 100.0,
        }
    }
}

/// Counted plays from which the rating is shown as medium or ok evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceParams {
    pub medium_from: u32,
    pub ok_from: u32,
}

impl Default for EvidenceParams {
    fn default() -> Self {
        Self {
            medium_from: 10,
            ok_from: 30,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceTier {
    Low,
    Medium,
    Ok,
}

impl EvidenceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::Ok => "ok",
        }
    }
}

impl EvidenceParams {
    pub fn tier(&self, counted: u32) -> EvidenceTier {
        if counted >= self.ok_from {
            EvidenceTier::Ok
        } else if counted >= self.medium_from {
            EvidenceTier::Medium
        } else {
            EvidenceTier::Low
        }
    }
}

/// ADR 0024: mods whose notes no longer match the chart MinaCalc rated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ExclusionParams {
    /// stable's column shuffle is not public (research 03).
    pub exclude_random: bool,
    pub exclude_coop: bool,
    /// Key conversion plays another keymode than the chart's.
    pub exclude_key_mod: bool,
}

impl Default for ExclusionParams {
    fn default() -> Self {
        Self {
            exclude_random: true,
            exclude_coop: true,
            exclude_key_mod: true,
        }
    }
}

impl ExclusionParams {
    pub fn params_hash(&self) -> [u8; 32] {
        params_hash(EXCLUSION_PARAMS_TAG, self)
    }
}

/// Rate tags that rate-copy tools append to a difficulty name: `1.15x`, `x1.2`, `(207bpm)`,
/// `[1.2x]`. Matching is ASCII case-insensitive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyParams {
    /// A number followed by one of these is a rate tag.
    pub number_suffixes: Vec<String>,
    /// One of these followed by a number is a rate tag.
    pub number_prefixes: Vec<String>,
    /// Open and close delimiters around a tag.
    pub brackets: Vec<(char, char)>,
}

impl Default for FamilyParams {
    fn default() -> Self {
        Self {
            number_suffixes: vec!["x".into(), "bpm".into()],
            number_prefixes: vec!["x".into()],
            brackets: vec![('(', ')'), ('[', ']')],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goal_defaults_match_adr_0024() {
        let p = GoalParams::default();
        assert_eq!(p.cap, 0.965);
        assert_eq!(p.timing_scale, 1.0);
        assert_eq!(p.simpson_intervals % 2, 0);
    }

    #[test]
    fn default_goal_params_hash_is_frozen() {
        let hex: String = GoalParams::default()
            .params_hash()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            hex,
            "f90e63ad14f179aef12dce4a881e737486f8ef8056fbbcb0f586874b706c3aee"
        );
    }

    #[test]
    fn goal_params_hash_moves_with_every_field() {
        let base = GoalParams::default();
        assert_eq!(base.params_hash(), GoalParams::default().params_hash());
        let variants = [
            GoalParams {
                cap: 0.96,
                ..base.clone()
            },
            GoalParams {
                timing_scale: 0.84,
                ..base.clone()
            },
            GoalParams {
                edge_pad_ms: 0.0,
                ..base.clone()
            },
            GoalParams {
                simpson_intervals: 64,
                ..base.clone()
            },
        ];
        for v in variants {
            assert_ne!(v.params_hash(), base.params_hash(), "{v:?}");
        }
    }

    #[test]
    fn evidence_tiers_split_at_the_thresholds() {
        let p = EvidenceParams {
            medium_from: 10,
            ok_from: 30,
        };
        assert_eq!(p.tier(0), EvidenceTier::Low);
        assert_eq!(p.tier(9), EvidenceTier::Low);
        assert_eq!(p.tier(10), EvidenceTier::Medium);
        assert_eq!(p.tier(29), EvidenceTier::Medium);
        assert_eq!(p.tier(30), EvidenceTier::Ok);
        assert_eq!(EvidenceTier::Medium.as_str(), "medium");
    }
}
