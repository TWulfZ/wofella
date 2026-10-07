//! Labelling DTOs (D13): camelCase on the wire, no 64-bit integers (spec 005). Times cross as
//! whole milliseconds, which is what osu! stores; the export file keeps microseconds.

use serde::{Deserialize, Serialize};

use crate::features::library::dto::ChartWindowDto;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PatternDefDto {
    pub id: String,
    pub axis: String,
    /// Short key for typing labels.
    pub key: String,
    pub description: String,
}

/// A synthetic chart that shows one pattern; `window.fromMs..=toMs` is the span to draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PatternExampleDto {
    pub id: String,
    pub window: ChartWindowDto,
}

/// `[t0Ms, t1Ms)` of one chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AnchorDto {
    pub md5: String,
    pub t0_ms: i32,
    pub t1_ms: i32,
    /// 1-based, column 1 leftmost, ascending.
    pub cols: Vec<u8>,
}

/// One labelling round. Label bounds are inclusive and match as in the library listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SampleRequestDto {
    pub keymode: u8,
    /// A `u64` in decimal: the sequence is a pure function of it and the labelled set.
    pub seed: String,
    pub round: u32,
    /// Defaults to the sampler's window.
    pub window_ms: Option<u32>,
    pub scale: Option<String>,
    pub level_min: Option<f64>,
    pub level_max: Option<f64>,
    /// Windows already shown this session (skips are not stored); labelled ones are always
    /// avoided.
    pub exclude: Vec<AnchorDto>,
}

/// A window inside one chart the user picked, start chosen like the sampler's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowAtRequestDto {
    pub keymode: u8,
    pub md5: String,
    /// A `u64` in decimal.
    pub seed: String,
    pub window_ms: Option<u32>,
    pub exclude: Vec<AnchorDto>,
}

/// Any eligible chart of the keymode, outside the stratified plan: it neither follows nor
/// consumes the sampler's rounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RandomRequestDto {
    pub keymode: u8,
    /// A `u64` in decimal.
    pub seed: String,
    pub round: u32,
    pub window_ms: Option<u32>,
    pub exclude: Vec<AnchorDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingRequestDto {
    pub keymode: u8,
    /// Windows already shown this session, so a skipped one is not offered again.
    pub exclude: Vec<AnchorDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NowPlayingDto {
    pub window: LabelWindowDto,
    pub source: NowPlayingSourceDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum NowPlayingSourceDto {
    /// osu! stable's window title named the chart.
    OsuWindow,
    /// The newest replay in `Data/r` played by the self profile.
    LastReplay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelWindowDto {
    pub anchor: AnchorDto,
    pub title: String,
    pub artist: String,
    pub version: String,
    pub creator: String,
    /// stable's cached no-mod star rating; `None` until stable has computed it.
    pub stars: Option<f32>,
    /// `scale:level` of the label that placed the chart in its stratum.
    pub level: Option<String>,
    /// e.g. `dan_07/nps_2`; display only, never persisted.
    pub stratum: String,
    pub played: bool,
}

/// `anchor` moved to start at `t0Ms`, keeping its length.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MoveWindowRequestDto {
    pub anchor: AnchorDto,
    pub t0_ms: i32,
}

/// `anchor` resized to `[t0Ms, t1Ms)`: the edge that moved from the anchor's is clamped to
/// the chart and to the window length bounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ResizeWindowRequestDto {
    pub anchor: AnchorDto,
    pub t0_ms: i32,
    pub t1_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChartTimelineRequestDto {
    pub keymode: u8,
    pub md5: String,
    pub buckets: u16,
}

/// A whole chart at a glance: where its notes are and which windows the user labelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChartTimelineDto {
    /// The first row.
    pub first_ms: i32,
    /// One past the last row: the latest a window may end.
    pub end_ms: i32,
    /// Notes (taps and LN heads) per equal slice of `[firstMs, endMs)`, earliest first.
    pub density: Vec<u16>,
    /// The self profile's labels on this chart that are not undone, by start.
    pub labelled: Vec<SpanDto>,
}

/// `[t0Ms, t1Ms)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SpanDto {
    pub t0_ms: i32,
    pub t1_ms: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelSubmitDto {
    pub anchor: AnchorDto,
    /// Full pattern ids of the chart's keymode: at least one, unless `no_pattern`.
    pub patterns: Vec<String>,
    /// "No clear pattern": a stored answer with empty `patterns` (a skip is never stored).
    pub no_pattern: bool,
    pub mixed: bool,
    pub unsure: bool,
    /// `None` is neutral.
    pub thumb_pref: Option<ThumbPrefDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ThumbPrefDto {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelEventDto {
    /// The feedback event's ULID; undo takes it back.
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CountDto {
    pub key: String,
    pub count: u32,
}

/// Over the self profile's labels that are not undone. Lists are sorted by key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelStatsDto {
    pub total: u32,
    /// Answers of "no clear pattern".
    pub no_pattern: u32,
    pub mixed: u32,
    pub unsure: u32,
    pub thumb_left: u32,
    pub thumb_right: u32,
    pub per_pattern: Vec<CountDto>,
    pub per_axis: Vec<CountDto>,
    /// A chart no longer in the library counts under `unknown`.
    pub per_stratum: Vec<CountDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelExportDto {
    pub path: String,
    pub rows: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum WindowOpDto {
    Widen,
    Narrow,
    Next,
    Prev,
}
