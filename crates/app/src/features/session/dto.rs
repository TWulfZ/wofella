//! Session DTOs (D13): camelCase on the wire, no 64-bit integers (spec 005).

use serde::{Deserialize, Serialize};

use crate::features::labeling::dto::SessionLabelDto;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlaysDto {
    /// When the app started; plays from then on belong to the session.
    pub started_at: String,
    /// Newest first; one row per play, so a map played twice is listed twice.
    pub plays: Vec<SessionPlayDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlayDto {
    /// Hex `PlayId`, as `session_label_submit` takes it back.
    pub play_id: String,
    pub md5: String,
    pub played_at: String,
    pub title: String,
    pub artist: String,
    pub version: String,
    pub creator: String,
    /// stable's cached no-mod star rating; `None` until stable has computed it.
    pub stars: Option<f32>,
    pub keymode: u8,
    /// osu! beatmapset id, for the website link; `None` for an unsubmitted map.
    pub set_id: Option<i32>,
    /// The chart's effective session answer, from this session or an earlier one.
    pub label: Option<SessionLabelDto>,
    /// The self profile's gold windows on the chart that no undo cancels. Information only:
    /// they never resolve the row (ADR 0020).
    pub gold_windows: u32,
}
