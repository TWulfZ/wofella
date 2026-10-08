//! `preview_skill` DTOs (ADR 0024, frozen in `odd/tasks/skill-preview.md`). camelCase on the wire,
//! snake_case enum values, no 64-bit integers (the bindings export fails on them, spec 005).

use serde::{Deserialize, Serialize};

pub const METHOD_ETTERNA_RATING: &str = "preview.etterna_rating@1";

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
    /// Some of the scope's plays have no SSR row yet, or `ComputePlaySsr` is queued or running.
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
    /// A `play_ssr` status other than `counted`.
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
