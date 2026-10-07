//! Skins DTOs (D13): camelCase on the wire, no 64-bit integers (spec 005). Lengths are in
//! stable's 480-high units; scaling to the canvas is the renderer's job (research 06).

use serde::{Deserialize, Serialize};

/// `R, G, B, A`, 0–255.
pub type RgbaDto = [u8; 4];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkinListDto {
    /// By folder name, byte order.
    pub skins: Vec<SkinEntryDto>,
    /// The cfg `Skin`, only when it names a listed folder byte for byte.
    pub current: Option<String>,
    /// The cfg `ManiaSpeed`, 1–40.
    pub mania_speed: Option<u8>,
    pub mania_speed_bpm_scale: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkinEntryDto {
    /// What `skin_get` takes; `[General] Name` is neither unique nor the folder (research 06).
    pub folder: String,
    pub name: Option<String>,
    /// Key counts with a `[Mania]` block; empty without a readable `skin.ini`.
    pub keymodes: Vec<u8>,
    /// Opaque cache key that changes when `skin.ini` is edited (ADR 0019).
    pub ini_mtime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkinDto {
    pub folder: String,
    pub name: Option<String>,
    /// `[General] Version`; 1.0 when absent, 2.7 for `latest` or without `skin.ini`.
    pub version: f64,
    pub config: ManiaConfigDto,
    /// Resolved slots only; a slot absent here is drawn procedurally.
    pub images: Vec<SkinImageRefDto>,
    /// Each resolved file once, shared by every slot that resolves to it.
    pub files: Vec<SkinFileDto>,
    pub diagnostics: Vec<SkinDiagnosticDto>,
    pub effects: SkinEffectsDto,
}

/// What the Label preview's autoplay effects need besides images, in stable's 480-high units
/// with lazer's defaults applied (research 06).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkinEffectsDto {
    /// `[Mania] ScorePosition`: hit-burst centre, y from the top.
    pub score_position: f32,
    /// `[Mania] ComboPosition`: combo counter centre, y from the top.
    pub combo_position: f32,
    /// Per column; 0 means the column width.
    pub lighting_n_width: Vec<f32>,
    pub lighting_l_width: Vec<f32>,
    /// `ColourLight1` is `[0]`, the stage light's tint; alpha 0 already sent as 255.
    pub light_colours: Vec<Option<RgbaDto>>,
    /// `[Fonts] ComboOverlap`: px the combo digits overlap; negative adds a gap.
    pub combo_overlap: f32,
}

/// The `[Mania]` block for the requested key count, or lazer's all-defaults block when the skin
/// has none (research 06).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ManiaConfigDto {
    pub keys: u8,
    pub column_width: Vec<f32>,
    /// `keys - 1` gaps.
    pub column_spacing: Vec<f32>,
    /// `keys + 1` lines.
    pub column_line_width: Vec<f32>,
    /// Judgement line y from the top, already clamped to 240–480.
    pub hit_position: f32,
    pub light_position: f32,
    pub width_for_note_height_scale: f32,
    pub note_body_style: NoteBodyStyleDto,
    pub judgement_line: bool,
    pub keys_under_notes: bool,
    pub upside_down: bool,
    pub barline_height: f32,
    pub colours: ManiaColoursDto,
}

/// lazer's values; the wiki's 0/1/2 disagree and are unverified (research 06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum NoteBodyStyleDto {
    Stretch,
    RepeatTop,
    RepeatBottom,
    RepeatTopAndBottom,
}

/// `None` where `skin.ini` sets no colour, so the renderer keeps its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ManiaColoursDto {
    /// `Colour1` is `column[0]`. Raw alpha: column backgrounds draw with alpha A², and alpha 0
    /// hides them (research 06, Geometry).
    pub column: Vec<Option<RgbaDto>>,
    /// Alpha 0 is already sent as 255, as stable does for these colours.
    pub column_line: Option<RgbaDto>,
    pub judgement_line: Option<RgbaDto>,
    pub barline: Option<RgbaDto>,
    pub hold: Option<RgbaDto>,
}

/// `slot` ids: `note.{i}`, `note.{i}.head`, `note.{i}.tail`, `body.{i}`, `key.{i}`,
/// `key.{i}.down`, `stage.{left,right,bottom,hint,light}`, with 0-based columns, and
/// `hit.{0,50,100,200,300,300g}`, `combo.{0-9}`, `lighting.{n,l}.{frame}` with 0-based frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkinImageRefDto {
    pub slot: String,
    /// Index into `SkinDto::files`.
    pub file: u16,
}

/// A PNG or JPEG checked by its header, never decoded here; the webview decodes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkinFileDto {
    pub mime: String,
    /// 2 for an `@2x` file: its display size is the pixel size halved.
    pub scale: u8,
    pub width: u32,
    pub height: u32,
    /// RFC 4648 with padding.
    pub base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkinDiagnosticDto {
    /// A stable `skin.*` id.
    pub code: String,
    pub slot: Option<String>,
}
