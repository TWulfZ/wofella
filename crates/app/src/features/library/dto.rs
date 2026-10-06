//! Library DTOs (D13): camelCase on the wire, no 64-bit integers (spec 005).

use serde::{Deserialize, Serialize};

/// Label bounds are inclusive, and any label criterion keeps only charts with a matching
/// label, ordered by scale and level; without one the order is by md5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFilterDto {
    pub keymode: u8,
    pub scale: Option<String>,
    pub level_min: Option<f64>,
    pub level_max: Option<f64>,
    pub label_source: Option<String>,
    /// Case-insensitive substring of title, artist, difficulty name or creator.
    pub text: Option<String>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChartLabelDto {
    pub source: String,
    pub scale: String,
    pub level_ord: Option<f64>,
    pub level_text: String,
    pub skill_tag: Option<String>,
    pub is_variant: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryChartDto {
    pub md5: String,
    pub title: String,
    pub artist: String,
    /// The difficulty name.
    pub version: String,
    pub creator: String,
    pub keymode: u8,
    pub n_notes: u32,
    pub n_ln: u32,
    pub ln_ratio: f64,
    pub length_ms: u32,
    pub nps: f64,
    pub labels: Vec<ChartLabelDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChartDetailDto {
    pub chart: LibraryChartDto,
    /// Decoder warnings of the `.osu` as it is now; `None` when the file is unavailable.
    pub diagnostics: Option<u32>,
    pub segments: Vec<SegmentDto>,
}

/// One pattern segment under the keymode profile's default layout, rows `t0Ms..=t1Ms`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SegmentDto {
    pub t0_ms: i32,
    pub t1_ms: i32,
    /// Bit `i` is column `i`.
    pub cols: u16,
    pub pattern_id: String,
    /// The taxonomy's short key.
    pub key: String,
    pub axis_id: String,
    pub secondary: Vec<String>,
    /// Permille.
    pub purity: u16,
    /// Permille.
    pub strength: u16,
}

/// Primary segments of one pattern over the library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PatternCountDto {
    pub keymode: u8,
    pub pattern_id: String,
    pub key: String,
    pub axis_id: String,
    pub segments: u32,
    pub charts: u32,
    pub total_s: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScaleCountDto {
    pub scale: String,
    pub rows: u32,
    pub charts: u32,
}

/// How far the engine's segments agree with one name-hint target over the library. Shares are
/// of a chart's segmented time; charts without segmented time have no share and are left out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HintAgreementDto {
    pub keymode: u8,
    /// `hint_pattern` or `hint_axis`.
    pub scale: String,
    pub target_id: String,
    /// Charts carrying the hint.
    pub hinted_charts: u32,
    /// Hinted charts with segmented time.
    pub segmented_charts: u32,
    /// Mean share of the target over the segmented hinted charts; a pattern counts its primary
    /// segments, an axis every segment on it.
    pub mean_share: Option<f64>,
    /// Mean share of the target over every segmented chart of the keymode.
    pub baseline_share: f64,
    /// `mean_share / baseline_share`; `None` without either.
    pub lift: Option<f64>,
}

/// What a playfield draws for `[fromMs, toMs]` of one chart. No segments: labelling is blind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChartWindowDto {
    pub md5: String,
    pub keymode: u8,
    pub from_ms: i32,
    pub to_ms: i32,
    /// Taps and LN heads in `[fromMs, toMs]` plus LNs whose body enters from before `fromMs`,
    /// by `(tMs, col)`.
    pub notes: Vec<NoteDto>,
    /// The last red line at or before `fromMs`, then every line in `[fromMs, toMs]`.
    pub timing: Vec<TimingDto>,
    pub layout: LayoutDto,
    pub chart_span: ChartSpanDto,
    pub audio_filename: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub t_ms: i32,
    /// 0-based, column 0 leftmost.
    pub col: u8,
    /// The LN tail; `None` for a tap.
    pub end_ms: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TimingKindDto {
    Red,
    Green,
}

/// A red line has `beatLenMs` and `meter`, a green line `sv`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TimingDto {
    pub t_ms: i32,
    pub kind: TimingKindDto,
    pub beat_len_ms: Option<f64>,
    pub meter: Option<u8>,
    pub sv: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LayoutDto {
    pub id: String,
    /// One per column, leftmost first.
    pub columns: Vec<ColumnDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDto {
    pub hand: HandDto,
    pub finger: FingerDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum HandDto {
    Left,
    Right,
    /// Either thumb may take it.
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum FingerDto {
    Pinky,
    Ring,
    Middle,
    Index,
    Thumb,
}

/// The first and the last row of the chart (LN tails included); `0, 0` without rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChartSpanDto {
    pub first_ms: i32,
    pub end_ms: i32,
}
