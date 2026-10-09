//! `preview_skill` and `preview_recs` DTOs (ADR 0024, frozen in `odd/tasks/skill-preview.md` and
//! `odd/tasks/recs-preview.md`). camelCase on the wire, snake_case enum values, no 64-bit
//! integers (the bindings export fails on them, spec 005).

use serde::{Deserialize, Serialize};

pub const METHOD_ETTERNA_RATING: &str = "preview.etterna_rating@1";
/// A play whose SSR row is missing while no `ComputePlaySsr` runs: an item failure or a job that
/// never ran under the current key.
pub const EXCLUSION_PENDING: &str = "pending";

pub mod warning {
    pub const UNCALIBRATED: &str = "uncalibrated";
    pub const GOAL_ESTIMATED: &str = "goal_estimated";
    pub const K7_LESS_VALIDATED: &str = "k7_less_validated";
    pub const LN_NOT_MEASURED: &str = "ln_not_measured";
    /// MinaCalc's 7K Technical sits near 0.18 on almost every chart.
    pub const K7_TECH_NOT_MEASURED: &str = "k7_tech_not_measured";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum PreviewStateDto {
    Ready,
    /// `ComputePlaySsr` is queued or running; the rows already cached are shown.
    Computing,
    NoPlays,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DanThirdDto {
    Low,
    Mid,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTierDto {
    Low,
    Medium,
    Ok,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkillsetRatingDto {
    /// A MinaCalc skillset id (`stream`, `jumpstream`, …), as `ChartMsdDto.skillsets` names them.
    pub id: String,
    pub rating_centi: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DanEstimateDto {
    pub label: String,
    pub third: DanThirdDto,
    pub margin_centi: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExclusionCountDto {
    /// A `play_ssr` status other than `counted`, or `pending` for a play with no row.
    pub reason: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceDto {
    pub counted: u32,
    pub tier: EvidenceTierDto,
    pub excluded: Vec<ExclusionCountDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TopPlayDto {
    /// 64 hex chars.
    pub play_id: String,
    pub md5: String,
    pub title: String,
    pub version: String,
    pub rate_milli: u16,
    pub goal_permyriad: u16,
    pub overall_centi: i32,
    pub dominant_skillset: String,
    /// Unix ms; an `f64` holds it exactly and crosses the bindings without a BigInt.
    pub played_at_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TrendPointDto {
    /// `YYYY-MM`, UTC.
    pub month: String,
    pub overall_centi: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkillPreviewDto {
    pub scope_hash: String,
    pub keymode: u8,
    pub method: String,
    pub calc_version: i32,
    pub state: PreviewStateDto,
    pub overall_centi: Option<i32>,
    pub skillsets: Vec<SkillsetRatingDto>,
    pub dan: Option<DanEstimateDto>,
    pub evidence: EvidenceDto,
    pub top_plays: Vec<TopPlayDto>,
    pub trend: Vec<TrendPointDto>,
    pub warnings: Vec<String>,
}

pub const METHOD_BAND_RECS: &str = "preview.band_recs@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RecsModeDto {
    /// The weakest skillset the keymode lets Deficit pick.
    Deficit,
    /// Overall, aimed above the player.
    Push,
    /// The skillset the caller names.
    Skillset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RecsStateDto {
    Ready,
    /// `ComputePlaySsr` is queued or running; the list reads the rating as cached so far.
    Computing,
    /// The scope has no rating in this keymode yet, so there is no band.
    NoRating,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ReasonDto {
    /// `deficit`, `push`, `skillset`, `unplayed`, `played_before`, `needs_rate_copy` or
    /// `rate_copy_in_library`: i18n keys.
    pub code: String,
    /// Skillsets as MinaCalc ids, MSD in centi, rates in milli.
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecItemDto {
    pub md5: String,
    pub title: String,
    pub artist: String,
    pub version: String,
    pub creator: String,
    pub set_id: Option<i32>,
    pub beatmap_id: Option<i32>,
    /// The mod rate to play at; a rate copy is always picked at 1000.
    pub rate_milli: u16,
    pub needs_rate_copy: bool,
    pub is_rate_copy: bool,
    pub focus_centi: i32,
    pub overall_centi: i32,
    /// The seven skillsets after Overall, in `SkillPreviewDto.skillsets` order.
    pub skillsets_centi: Vec<i32>,
    pub played: bool,
    pub reasons: Vec<ReasonDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecsPreviewDto {
    /// Empty when the entry resolves to no scope.
    pub scope_hash: String,
    pub keymode: u8,
    pub method: String,
    pub calc_version: i32,
    pub state: RecsStateDto,
    pub any_rate: bool,
    /// A MinaCalc skillset id (`overall` for Push); empty for Deficit without a rating.
    pub focus: String,
    /// The scope's rating on `focus`; 0 without a rating.
    pub rating_centi: i32,
    /// Inclusive absolute MSD band; `[0, 0]` without a rating.
    pub band_centi: [i32; 2],
    pub items: Vec<RecItemDto>,
    pub warnings: Vec<String>,
}
