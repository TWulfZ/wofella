//! osu! stable `skin.ini` reader for the `[General]` identity and the `[Mania]` blocks. stable's
//! source is closed, so this ports lazer's legacy decoders (MIT, ppy/osu @
//! 6359741babba4ced42da1fcda738c35370fdf3f4):
//!
//! - lines: `osu.Game/Beatmaps/Formats/LegacyDecoder.cs` L45-67 (loop), L81 (blank and `//`
//!   lines), L101-108 (trailing `//` comments), L155-164 (`Key: Value` split), L110-128 (colours);
//! - `[General]`: `osu.Game/Skinning/LegacySkinDecoder.cs` L35-47 (Version, `latest`), L66-72
//!   (missing Version is 1.0), `osu.Game/Skinning/SkinConfiguration.cs` L17 (latest is 2.7);
//! - `[Mania]`: `osu.Game/Skinning/LegacyManiaSkinDecoder.cs` L25-31 (a section resets the block),
//!   L43-60 (lines before `Keys:` are held, first duplicate block wins), L77-191 (keys),
//!   L197-216 (an unparsable list entry is 0);
//! - defaults: `osu.Game/Skinning/LegacyManiaSkinConfiguration.cs` L17-19, L35-42, L62-75;
//!   `osu.Game/Skinning/LegacySkin.cs` L141-144 (missing block), L184 (`Colour` is 1-based),
//!   L214/L233 (images are 0-based), L204-210 (body style by version);
//! - colours: `osu.Game/Skinning/LegacyColourCompatibility.cs` L22-27 (alpha 0);
//! - playback effects: `LegacyManiaSkinDecoder.cs` L101-107 (`ComboPosition`, `ScorePosition`),
//!   L117-123 (`LightingNWidth`, `LightingLWidth`), L180 (`Hit*` image keys),
//!   `LegacyManiaSkinConfiguration.cs` L37-38 (their defaults), `LegacySkin.cs` L186-188
//!   (`ColourLight` is 1-based); `[Fonts]` `ComboPrefix` / `ComboOverlap`:
//!   `osu.Game/Skinning/LegacySkinExtensions.cs` L150-151, L176-177.
//!
//! Lengths stay in stable's 480-high units; lazer's ×1.6 belongs to rendering. Deliberate
//! deviations: section names match case-insensitively and an unknown section is ignored (lazer:
//! case-sensitive, unknown falls back to General); `[Fonts]` keys are read only in that section
//! (lazer keeps any section's unknown key in one dictionary); `Keys` above `max_keys` discards its
//! block instead of allocating it; non-finite numbers count as unparsable; `NoteBodyStyle: 1` is
//! `RepeatBottom` and other values outside lazer's enum are ignored. Text is UTF-8 with or without
//! BOM, or UTF-16 with a BOM, like lazer's `StreamReader`; invalid UTF-8 is decoded lossily and
//! flagged in `decoded_lossily`.

use std::collections::BTreeMap;

/// Format defaults and caps from lazer (module doc), not tunables; `max_keys` bounds allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinIniParams {
    pub default_version: f64,
    pub latest_version: f64,
    pub repeat_bottom_from_version: f64,
    pub max_keys: u8,
    pub column_width: f32,
    pub column_spacing: f32,
    pub column_line_width: f32,
    pub hit_position: f32,
    pub hit_position_min: f32,
    pub hit_position_max: f32,
    pub light_position: f32,
    pub barline_height: f32,
    pub score_position: f32,
    pub combo_position: f32,
}

impl Default for SkinIniParams {
    fn default() -> Self {
        Self {
            default_version: 1.0,
            latest_version: 2.7,
            repeat_bottom_from_version: 2.5,
            max_keys: 18,
            column_width: 30.0,
            column_spacing: 0.0,
            column_line_width: 2.0,
            hit_position: 402.0,
            hit_position_min: 240.0,
            hit_position_max: 480.0,
            light_position: 413.0,
            barline_height: 1.2,
            score_position: 300.0,
            combo_position: 111.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkinIni {
    pub name: Option<String>,
    pub version: f64,
    pub mania: BTreeMap<u8, ManiaConfig>,
    pub fonts: SkinFonts,
    pub decoded_lossily: bool,
}

/// `[Fonts]`; `None` leaves lazer's default (prefix `score`, overlap 0) to the reader.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkinFonts {
    pub combo_prefix: Option<String>,
    pub combo_overlap: Option<f32>,
}

impl SkinIni {
    /// lazer gives a skin folder without `skin.ini` the latest version (`Skin.cs` L110-117).
    pub fn absent(params: &SkinIniParams) -> Self {
        Self {
            name: None,
            version: params.latest_version,
            mania: BTreeMap::new(),
            fonts: SkinFonts::default(),
            decoded_lossily: false,
        }
    }

    pub fn mania(&self, keys: u8) -> Option<&ManiaConfig> {
        self.mania.get(&keys)
    }

    /// lazer builds an all-defaults block for a key count the skin does not define and still
    /// uses the skin's default-named images (`LegacySkin.cs` L141-144).
    pub fn mania_or_default(&self, keys: u8, params: &SkinIniParams) -> ManiaConfig {
        self.mania
            .get(&keys)
            .cloned()
            .unwrap_or_else(|| ManiaConfig::defaults(keys, self.version, params))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteBodyStyle {
    Stretch,
    RepeatTop,
    RepeatBottom,
    RepeatTopAndBottom,
}

impl NoteBodyStyle {
    fn by_version(version: f64, params: &SkinIniParams) -> Self {
        if version < params.repeat_bottom_from_version {
            Self::Stretch
        } else {
            Self::RepeatBottom
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    /// stable forces alpha 0 to opaque when it sets a colour after construction; column
    /// backgrounds instead use the raw alpha as drawable alpha, so the parsed value stays raw.
    pub fn disallow_zero_alpha(self) -> Self {
        Self {
            a: if self.a == 0 { u8::MAX } else { self.a },
            ..self
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManiaColours {
    /// `Colour1` is `column[0]`.
    pub column: Vec<Option<Rgba>>,
    pub column_line: Option<Rgba>,
    pub judgement_line: Option<Rgba>,
    pub barline: Option<Rgba>,
    pub hold: Option<Rgba>,
    /// `ColourLight1` is `light[0]`: the stage light's tint per column.
    pub light: Vec<Option<Rgba>>,
}

/// `Hit0` … `Hit300g` paths: the judgement bursts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitImages {
    pub h0: Option<String>,
    pub h50: Option<String>,
    pub h100: Option<String>,
    pub h200: Option<String>,
    pub h300: Option<String>,
    pub h300g: Option<String>,
}

/// `NoteImage0` / `KeyImage0` are column 0. Paths keep the raw text with `\` turned into `/`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColumnImages {
    pub note: Option<String>,
    pub head: Option<String>,
    pub body: Option<String>,
    pub tail: Option<String>,
    pub key: Option<String>,
    pub key_down: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StageImages {
    pub left: Option<String>,
    pub right: Option<String>,
    pub bottom: Option<String>,
    pub hint: Option<String>,
    pub light: Option<String>,
    pub lighting_n: Option<String>,
    pub lighting_l: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ManiaConfig {
    pub keys: u8,
    pub column_width: Vec<f32>,
    /// `keys - 1` gaps.
    pub column_spacing: Vec<f32>,
    /// `keys + 1` lines.
    pub column_line_width: Vec<f32>,
    /// Judgement line y from the top, already clamped.
    pub hit_position: f32,
    pub light_position: f32,
    /// Absent or ≤ 0 means the narrowest column (`LegacySkin.cs` L152-156).
    pub width_for_note_height_scale: Option<f32>,
    pub note_body_style: NoteBodyStyle,
    pub judgement_line: bool,
    pub keys_under_notes: bool,
    pub upside_down: bool,
    pub barline_height: f32,
    pub colours: ManiaColours,
    pub columns: Vec<ColumnImages>,
    pub stage: StageImages,
    pub hits: HitImages,
    /// Hit-burst centre, y from the top in 480-high units.
    pub score_position: f32,
    /// Combo counter centre, y from the top in 480-high units.
    pub combo_position: f32,
    /// Per column; 0 means the column width (`LegacySkin.cs` L296-299).
    pub lighting_n_width: Vec<f32>,
    pub lighting_l_width: Vec<f32>,
}

impl ManiaConfig {
    pub fn defaults(keys: u8, version: f64, params: &SkinIniParams) -> Self {
        let n = usize::from(keys);
        Self {
            keys,
            column_width: vec![params.column_width; n],
            column_spacing: vec![params.column_spacing; n.saturating_sub(1)],
            column_line_width: vec![params.column_line_width; n + 1],
            hit_position: params.hit_position,
            light_position: params.light_position,
            width_for_note_height_scale: None,
            note_body_style: NoteBodyStyle::by_version(version, params),
            judgement_line: true,
            keys_under_notes: false,
            upside_down: false,
            barline_height: params.barline_height,
            colours: ManiaColours {
                column: vec![None; n],
                light: vec![None; n],
                ..ManiaColours::default()
            },
            columns: vec![ColumnImages::default(); n],
            stage: StageImages::default(),
            hits: HitImages::default(),
            score_position: params.score_position,
            combo_position: params.combo_position,
            lighting_n_width: vec![0.0; n],
            lighting_l_width: vec![0.0; n],
        }
    }
}

pub fn parse_skin_ini(bytes: &[u8]) -> SkinIni {
    parse_skin_ini_with(bytes, &SkinIniParams::default())
}

pub fn parse_skin_ini_with(bytes: &[u8], params: &SkinIniParams) -> SkinIni {
    let (text, decoded_lossily) = decode(bytes);
    let mut parser = Parser {
        params,
        section: Section::General,
        name: None,
        version: params.default_version,
        blocks: BTreeMap::new(),
        fonts: SkinFonts::default(),
        current: Current::None,
        pending: Vec::new(),
    };
    for line in text.split(['\r', '\n']) {
        parser.line(line);
    }
    let version = parser.version;
    let mania = parser
        .blocks
        .into_iter()
        .map(|(keys, (mut config, explicit))| {
            config.note_body_style =
                explicit.unwrap_or_else(|| NoteBodyStyle::by_version(version, params));
            (keys, config)
        })
        .collect();
    SkinIni {
        name: parser.name,
        version,
        mania,
        fonts: parser.fonts,
        decoded_lossily,
    }
}

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const UTF16_LE_BOM: &[u8] = b"\xff\xfe";
const UTF16_BE_BOM: &[u8] = b"\xfe\xff";
const COMMENT: &str = "//";
const LATEST: &str = "latest";

fn decode(bytes: &[u8]) -> (String, bool) {
    let utf16 = |rest: &[u8], unit: fn([u8; 2]) -> u16| {
        let units = rest.chunks(2).map(|c| match *c {
            [a, b] => unit([a, b]),
            _ => 0xfffd,
        });
        let mut lossy = !rest.len().is_multiple_of(2);
        let text = char::decode_utf16(units)
            .map(|c| {
                c.unwrap_or_else(|_| {
                    lossy = true;
                    char::REPLACEMENT_CHARACTER
                })
            })
            .collect();
        (text, lossy)
    };
    if let Some(rest) = bytes.strip_prefix(UTF8_BOM) {
        utf8(rest)
    } else if let Some(rest) = bytes.strip_prefix(UTF16_LE_BOM) {
        utf16(rest, u16::from_le_bytes)
    } else if let Some(rest) = bytes.strip_prefix(UTF16_BE_BOM) {
        utf16(rest, u16::from_be_bytes)
    } else {
        utf8(bytes)
    }
}

fn utf8(bytes: &[u8]) -> (String, bool) {
    match String::from_utf8_lossy(bytes) {
        std::borrow::Cow::Borrowed(s) => (s.to_owned(), false),
        std::borrow::Cow::Owned(s) => (s, true),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Mania,
    Fonts,
    Other,
}

impl Section {
    fn from_header(name: &str) -> Self {
        if name.eq_ignore_ascii_case("General") {
            Self::General
        } else if name.eq_ignore_ascii_case("Mania") {
            Self::Mania
        } else if name.eq_ignore_ascii_case("Fonts") {
            Self::Fonts
        } else {
            Self::Other
        }
    }
}

/// The block that `[Mania]` lines currently flow into. `Discarded` stands for lazer's
/// configuration that is built for a duplicate `Keys` but never added to the output.
#[derive(Clone, Copy)]
enum Current {
    None,
    Block(u8),
    Discarded,
}

struct Parser<'a> {
    params: &'a SkinIniParams,
    section: Section,
    name: Option<String>,
    version: f64,
    blocks: BTreeMap<u8, (ManiaConfig, Option<NoteBodyStyle>)>,
    fonts: SkinFonts,
    current: Current,
    pending: Vec<String>,
}

fn split_key_value(line: &str) -> (&str, &str) {
    match line.split_once(':') {
        Some((k, v)) => (k.trim(), v.trim()),
        None => (line.trim(), ""),
    }
}

impl Parser<'_> {
    fn line(&mut self, raw: &str) {
        if raw.trim().is_empty() || raw.trim_start().starts_with(COMMENT) {
            return;
        }
        let line = raw.find(COMMENT).map_or(raw, |i| &raw[..i]).trim_end();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            self.section = Section::from_header(name);
            self.pending.clear();
            self.current = Current::None;
            return;
        }
        match self.section {
            Section::General => self.general(line),
            Section::Mania => self.mania(line),
            Section::Fonts => self.fonts(line),
            Section::Other => {}
        }
    }

    fn general(&mut self, line: &str) {
        match split_key_value(line) {
            ("Name", value) => self.name = (!value.is_empty()).then(|| value.to_owned()),
            ("Version", LATEST) => self.version = self.params.latest_version,
            ("Version", value) => {
                if let Some(v) = parse_version(value) {
                    self.version = v;
                }
            }
            _ => {}
        }
    }

    fn fonts(&mut self, line: &str) {
        match split_key_value(line) {
            ("ComboPrefix", value) => {
                self.fonts.combo_prefix = (!value.is_empty()).then(|| value.replace('\\', "/"));
            }
            ("ComboOverlap", value) => {
                if let Some(v) = parse_f32(value) {
                    self.fonts.combo_overlap = Some(v);
                }
            }
            _ => {}
        }
    }

    fn mania(&mut self, line: &str) {
        let (key, value) = split_key_value(line);
        if key != "Keys" {
            self.pending.push(line.to_owned());
            if !matches!(self.current, Current::None) {
                self.flush();
            }
            return;
        }
        // lazer aborts the line on an unparsable or non-positive count, keeping the previous block.
        let Some(keys) = value.parse::<i32>().ok().filter(|&k| k > 0) else {
            return;
        };
        self.current = match u8::try_from(keys) {
            Ok(k) if k <= self.params.max_keys && !self.blocks.contains_key(&k) => {
                let version = self.version;
                self.blocks
                    .insert(k, (ManiaConfig::defaults(k, version, self.params), None));
                Current::Block(k)
            }
            _ => Current::Discarded,
        };
        self.flush();
    }

    fn flush(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        let Current::Block(keys) = self.current else {
            return;
        };
        let Some((config, style)) = self.blocks.get_mut(&keys) else {
            return;
        };
        for line in &pending {
            apply(config, style, line, self.params);
        }
    }
}

fn parse_version(value: &str) -> Option<f64> {
    let digits = value.chars().filter(char::is_ascii_digit).count();
    let dots = value.chars().filter(|&c| c == '.').count();
    (digits > 0 && dots <= 1 && digits + dots == value.chars().count())
        .then(|| value.parse::<f64>().ok())
        .flatten()
}

fn parse_f32(value: &str) -> Option<f32> {
    value.trim().parse::<f32>().ok().filter(|v| v.is_finite())
}

fn parse_list(value: &str, out: &mut [f32]) {
    for (slot, entry) in out.iter_mut().zip(value.split(',')) {
        *slot = parse_f32(entry).unwrap_or(0.0);
    }
}

fn parse_colour(value: &str) -> Option<Rgba> {
    let parts = value
        .split(',')
        .map(|p| p.trim().parse::<u8>().ok())
        .collect::<Option<Vec<u8>>>()?;
    match parts[..] {
        [r, g, b] => Some(Rgba {
            r,
            g,
            b,
            a: u8::MAX,
        }),
        [r, g, b, a] => Some(Rgba { r, g, b, a }),
        _ => None,
    }
}

fn parse_body_style(value: &str) -> Option<NoteBodyStyle> {
    match value {
        "0" | "Stretch" => Some(NoteBodyStyle::Stretch),
        // Wiki "Repeat": lazer's numeric `Enum.TryParse` keeps it and draws it as a repeat style.
        "1" => Some(NoteBodyStyle::RepeatBottom),
        "2" | "RepeatTop" => Some(NoteBodyStyle::RepeatTop),
        "3" | "RepeatBottom" => Some(NoteBodyStyle::RepeatBottom),
        "4" | "RepeatTopAndBottom" => Some(NoteBodyStyle::RepeatTopAndBottom),
        _ => None,
    }
}

/// lazer stores image and colour keys verbatim and looks them up by `format!("{prefix}{i}")`, so
/// only the canonical spelling of an index matches (`NoteImage01` never does).
fn indexed<'k>(key: &'k str, prefix: &str) -> Option<(usize, &'k str)> {
    let rest = key.strip_prefix(prefix)?;
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let (digits, suffix) = rest.split_at(end);
    let index = digits.parse::<usize>().ok()?;
    (index.to_string() == digits).then_some((index, suffix))
}

fn image_path(value: &str) -> Option<String> {
    Some(value.replace('\\', "/"))
}

fn apply(
    config: &mut ManiaConfig,
    style: &mut Option<NoteBodyStyle>,
    line: &str,
    params: &SkinIniParams,
) {
    let (key, value) = split_key_value(line);
    match key {
        "ColumnWidth" => parse_list(value, &mut config.column_width),
        "ColumnSpacing" => parse_list(value, &mut config.column_spacing),
        "ColumnLineWidth" => parse_list(value, &mut config.column_line_width),
        "BarlineHeight" => set(&mut config.barline_height, parse_f32(value)),
        "HitPosition" => set(
            &mut config.hit_position,
            parse_f32(value).map(|v| v.max(params.hit_position_min).min(params.hit_position_max)),
        ),
        "LightPosition" => set(&mut config.light_position, parse_f32(value)),
        "ScorePosition" => set(&mut config.score_position, parse_f32(value)),
        "ComboPosition" => set(&mut config.combo_position, parse_f32(value)),
        "LightingNWidth" => parse_list(value, &mut config.lighting_n_width),
        "LightingLWidth" => parse_list(value, &mut config.lighting_l_width),
        "WidthForNoteHeightScale" => {
            if let Some(v) = parse_f32(value) {
                config.width_for_note_height_scale = Some(v);
            }
        }
        "JudgementLine" => config.judgement_line = value == "1",
        "KeysUnderNotes" => config.keys_under_notes = value == "1",
        "UpsideDown" => config.upside_down = value == "1",
        "NoteBodyStyle" => {
            if let Some(s) = parse_body_style(value) {
                *style = Some(s);
            }
        }
        "StageLeft" => config.stage.left = image_path(value),
        "StageRight" => config.stage.right = image_path(value),
        "StageBottom" => config.stage.bottom = image_path(value),
        "StageHint" => config.stage.hint = image_path(value),
        "StageLight" => config.stage.light = image_path(value),
        "LightingN" => config.stage.lighting_n = image_path(value),
        "LightingL" => config.stage.lighting_l = image_path(value),
        "Hit0" => config.hits.h0 = image_path(value),
        "Hit50" => config.hits.h50 = image_path(value),
        "Hit100" => config.hits.h100 = image_path(value),
        "Hit200" => config.hits.h200 = image_path(value),
        "Hit300" => config.hits.h300 = image_path(value),
        "Hit300g" => config.hits.h300g = image_path(value),
        _ if key.starts_with("Colour") => apply_colour(&mut config.colours, key, value),
        _ => apply_column_image(&mut config.columns, key, value),
    }
}

fn set(slot: &mut f32, value: Option<f32>) {
    if let Some(v) = value {
        *slot = v;
    }
}

fn apply_colour(colours: &mut ManiaColours, key: &str, value: &str) {
    let Some(colour) = parse_colour(value) else {
        return;
    };
    let slot = match key {
        "ColourColumnLine" => &mut colours.column_line,
        "ColourJudgementLine" => &mut colours.judgement_line,
        "ColourBarline" => &mut colours.barline,
        "ColourHold" => &mut colours.hold,
        _ if key.starts_with("ColourLight") => match indexed(key, "ColourLight") {
            Some((n, "")) if n >= 1 => match colours.light.get_mut(n - 1) {
                Some(slot) => slot,
                None => return,
            },
            _ => return,
        },
        _ => match indexed(key, "Colour") {
            Some((n, "")) if n >= 1 => match colours.column.get_mut(n - 1) {
                Some(slot) => slot,
                None => return,
            },
            _ => return,
        },
    };
    *slot = Some(colour);
}

fn apply_column_image(columns: &mut [ColumnImages], key: &str, value: &str) {
    let (index, field): (usize, fn(&mut ColumnImages) -> &mut Option<String>) =
        if let Some((i, suffix)) = indexed(key, "NoteImage") {
            match suffix {
                "" => (i, |c| &mut c.note),
                "H" => (i, |c| &mut c.head),
                "L" => (i, |c| &mut c.body),
                "T" => (i, |c| &mut c.tail),
                _ => return,
            }
        } else if let Some((i, suffix)) = indexed(key, "KeyImage") {
            match suffix {
                "" => (i, |c| &mut c.key),
                "D" => (i, |c| &mut c.key_down),
                _ => return,
            }
        } else {
            return;
        };
    if let Some(column) = columns.get_mut(index) {
        *field(column) = image_path(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn mania(text: &str, keys: u8) -> ManiaConfig {
        parse_skin_ini(text.as_bytes())
            .mania(keys)
            .cloned()
            .unwrap_or_else(|| panic!("no Keys: {keys} block"))
    }

    fn rgba(r: u8, g: u8, b: u8, a: u8) -> Option<Rgba> {
        Some(Rgba { r, g, b, a })
    }

    #[test]
    fn bom_and_crlf() {
        let ini = parse_skin_ini(
            b"\xef\xbb\xbf[General]\r\nName: Test Skin\r\nVersion: 2.5\r\n\r\n[Mania]\r\nKeys: 7\r\nHitPosition: 428\r\n",
        );
        assert_eq!(ini.name.as_deref(), Some("Test Skin"));
        assert_eq!(ini.version, 2.5);
        assert_eq!(ini.mania(7).map(|m| m.hit_position), Some(428.0));
        assert!(!ini.decoded_lossily);
    }

    #[test]
    fn lone_cr_and_lf_line_endings() {
        let ini = parse_skin_ini(b"[General]\rName: A\rVersion: 2.4\n[Mania]\nKeys: 4\n");
        assert_eq!(ini.name.as_deref(), Some("A"));
        assert_eq!(ini.version, 2.4);
        assert!(ini.mania(4).is_some());
    }

    #[test]
    fn utf16_with_bom() {
        let text = "[General]\r\nName: 스킨\r\n";
        let mut le = vec![0xff, 0xfe];
        let mut be = vec![0xfe, 0xff];
        for unit in text.encode_utf16() {
            le.extend_from_slice(&unit.to_le_bytes());
            be.extend_from_slice(&unit.to_be_bytes());
        }
        assert_eq!(parse_skin_ini(&le).name.as_deref(), Some("스킨"));
        assert_eq!(parse_skin_ini(&be).name.as_deref(), Some("스킨"));
    }

    #[test]
    fn invalid_utf8_is_lossy_and_flagged() {
        let ini = parse_skin_ini(b"[General]\nName: a\xffb\nVersion: 2.7\n");
        assert_eq!(ini.name.as_deref(), Some("a\u{fffd}b"));
        assert_eq!(ini.version, 2.7);
        assert!(ini.decoded_lossily);
    }

    #[test]
    fn comments_are_stripped() {
        let ini = parse_skin_ini(
            b"// leading comment\n[General] // c\nName: A // trailing\n   // indented: x\nVersion: 2.7//x\n[Mania]\n//Keys: 4\nKeys: 7\n",
        );
        assert_eq!(ini.name.as_deref(), Some("A"));
        assert_eq!(ini.version, 2.7);
        assert_eq!(ini.mania.keys().copied().collect::<Vec<_>>(), vec![7]);
    }

    #[test]
    fn section_names_case_insensitive_unknown_ignored() {
        let ini =
            parse_skin_ini(b"[MANIA]\nKeys: 4\n[Fonts]\nName: wrong\n[whatever]\nVersion: 2.5\n");
        assert!(ini.mania(4).is_some());
        assert_eq!(ini.name, None);
        assert_eq!(ini.version, 1.0);
        let ini = parse_skin_ini(b"Name: before any section\n");
        assert_eq!(ini.name.as_deref(), Some("before any section"));
    }

    #[test]
    fn lines_before_keys_belong_to_the_block() {
        let m = mania(
            "[Mania]\nColumnWidth: 40,41,42,43\nHitPosition: 410\nKeys: 4\nJudgementLine: 0\n",
            4,
        );
        assert_eq!(m.column_width, vec![40.0, 41.0, 42.0, 43.0]);
        assert_eq!(m.hit_position, 410.0);
        assert!(!m.judgement_line);
    }

    #[test]
    fn keys_switches_block_within_a_section() {
        let ini =
            parse_skin_ini(b"[Mania]\nKeys: 4\nHitPosition: 420\nKeys: 7\nHitPosition: 430\n");
        assert_eq!(ini.mania(4).map(|m| m.hit_position), Some(420.0));
        assert_eq!(ini.mania(7).map(|m| m.hit_position), Some(430.0));
    }

    #[test]
    fn new_section_discards_held_lines() {
        let m = mania("[Mania]\nHitPosition: 300\n[Mania]\nKeys: 7\n", 7);
        assert_eq!(m.hit_position, 402.0);
    }

    #[test]
    fn duplicate_block_first_wins() {
        let m = mania(
            "[Mania]\nKeys: 7\nHitPosition: 410\n[Mania]\nHitPosition: 450\nKeys: 7\nColumnWidth: 50\n",
            7,
        );
        assert_eq!(m.hit_position, 410.0);
        assert_eq!(m.column_width, vec![30.0; 7]);
    }

    #[test]
    fn invalid_keys_lines() {
        let ini = parse_skin_ini(b"[Mania]\nKeys: 4\nKeys: x\nKeys: 0\nHitPosition: 300\n");
        assert_eq!(ini.mania.keys().copied().collect::<Vec<_>>(), vec![4]);
        assert_eq!(ini.mania(4).map(|m| m.hit_position), Some(300.0));
        let ini = parse_skin_ini(b"[Mania]\nKeys: 4\nKeys: 19\nHitPosition: 300\n");
        assert_eq!(ini.mania.keys().copied().collect::<Vec<_>>(), vec![4]);
        assert_eq!(ini.mania(4).map(|m| m.hit_position), Some(402.0));
    }

    #[test]
    fn broken_list_entries_are_zero() {
        let m = mania(
            "[Mania]\nKeys: 5\nColumnWidth: 40, abc,,42 ,inf,99\nColumnLineWidth: 1,2\nColumnSpacing: 3\n",
            5,
        );
        assert_eq!(m.column_width, vec![40.0, 0.0, 0.0, 42.0, 0.0]);
        assert_eq!(m.column_line_width, vec![1.0, 2.0, 2.0, 2.0, 2.0, 2.0]);
        assert_eq!(m.column_spacing, vec![3.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn images_zero_based_colours_one_based() {
        let m = mania(
            "[Mania]\nKeys: 7\nNoteImage0: n0\nNoteImage6H: h6\nNoteImage6L: l6\nNoteImage6T: t6\n\
             NoteImage7: out\nNoteImage01: padded\nKeyImage0: k0\nKeyImage0D: kd0\n\
             Colour1: 10,20,30\nColour7: 1,2,3,4\nColour8: 9,9,9\nColour0: 9,9,9\n",
            7,
        );
        assert_eq!(m.columns.len(), 7);
        assert_eq!(m.columns[0].note.as_deref(), Some("n0"));
        assert_eq!(m.columns[1].note, None);
        assert_eq!(m.columns[6].head.as_deref(), Some("h6"));
        assert_eq!(m.columns[6].body.as_deref(), Some("l6"));
        assert_eq!(m.columns[6].tail.as_deref(), Some("t6"));
        assert_eq!(m.columns[6].note, None);
        assert_eq!(m.columns[0].key.as_deref(), Some("k0"));
        assert_eq!(m.columns[0].key_down.as_deref(), Some("kd0"));
        assert_eq!(m.colours.column.len(), 7);
        assert_eq!(m.colours.column[0], rgba(10, 20, 30, 255));
        assert_eq!(m.colours.column[6], rgba(1, 2, 3, 4));
        assert!(m.colours.column[1..6].iter().all(Option::is_none));
    }

    #[test]
    fn stage_images_and_backslash_paths() {
        let m = mania(
            "[Mania]\nKeys: 7\nNoteImage0L: Notes4K\\mania-note1L\nStageHint: mania\\stage\\hint\n\
             StageLeft: l\nStageRight: r\nStageBottom: b\nStageLight: li\nLightingN: 7k/LightingN\n\
             LightingL: 7k\\LightingL\n",
            7,
        );
        assert_eq!(m.columns[0].body.as_deref(), Some("Notes4K/mania-note1L"));
        assert_eq!(m.stage.hint.as_deref(), Some("mania/stage/hint"));
        assert_eq!(m.stage.left.as_deref(), Some("l"));
        assert_eq!(m.stage.right.as_deref(), Some("r"));
        assert_eq!(m.stage.bottom.as_deref(), Some("b"));
        assert_eq!(m.stage.light.as_deref(), Some("li"));
        assert_eq!(m.stage.lighting_n.as_deref(), Some("7k/LightingN"));
        assert_eq!(m.stage.lighting_l.as_deref(), Some("7k/LightingL"));
    }

    #[test]
    fn version_defaults() {
        for (text, want) in [
            ("", 1.0),
            ("[General]\nVersion: latest\n", 2.7),
            ("[General]\nVersion: Latest\n", 1.0),
            ("[General]\nVersion: 3\n", 3.0),
            ("[General]\nVersion: 2.4\n", 2.4),
            ("[General]\nVersion: abc\n", 1.0),
            ("[General]\nVersion: -2\n", 1.0),
        ] {
            assert_eq!(parse_skin_ini(text.as_bytes()).version, want, "{text:?}");
        }
    }

    #[test]
    fn note_body_style_default_by_version() {
        let style = |text: &str| mania(text, 7).note_body_style;
        assert_eq!(style("[Mania]\nKeys: 7\n"), NoteBodyStyle::Stretch);
        assert_eq!(
            style("[General]\nVersion: 2.4\n[Mania]\nKeys: 7\n"),
            NoteBodyStyle::Stretch
        );
        assert_eq!(
            style("[General]\nVersion: 2.5\n[Mania]\nKeys: 7\n"),
            NoteBodyStyle::RepeatBottom
        );
        assert_eq!(
            style("[Mania]\nKeys: 7\n[General]\nVersion: latest\n"),
            NoteBodyStyle::RepeatBottom
        );
        assert_eq!(
            style("[General]\nVersion: 2.5\n[Mania]\nKeys: 7\nNoteBodyStyle: 2\n"),
            NoteBodyStyle::RepeatTop
        );
        assert_eq!(
            style("[General]\nVersion: 2.5\n[Mania]\nKeys: 7\nNoteBodyStyle: 0\n"),
            NoteBodyStyle::Stretch
        );
        assert_eq!(
            style("[Mania]\nKeys: 7\nNoteBodyStyle: RepeatTopAndBottom\n"),
            NoteBodyStyle::RepeatTopAndBottom
        );
        assert_eq!(
            style("[General]\nVersion: 2.5\n[Mania]\nKeys: 7\nNoteBodyStyle: 1\n"),
            NoteBodyStyle::RepeatBottom
        );
    }

    #[test]
    fn note_body_style_one_is_explicit_repeat() {
        let style = |text: &str| mania(text, 7).note_body_style;
        for version in ["", "[General]\nVersion: 1.0\n", "[General]\nVersion: 2.4\n"] {
            assert_eq!(
                style(&format!("{version}[Mania]\nKeys: 7\nNoteBodyStyle: 1\n")),
                NoteBodyStyle::RepeatBottom,
                "{version:?}"
            );
        }
    }

    #[test]
    fn hit_position_clamped() {
        for (raw, want) in [("100", 240.0), ("500", 480.0), ("428", 428.0), ("x", 402.0)] {
            let m = mania(&format!("[Mania]\nKeys: 7\nHitPosition: {raw}\n"), 7);
            assert_eq!(m.hit_position, want, "HitPosition: {raw}");
        }
    }

    #[test]
    fn scalar_fields() {
        let m = mania(
            "[Mania]\nKeys: 7\nLightPosition: 400\nWidthForNoteHeightScale: 42\nBarlineHeight: 2.5\n\
             KeysUnderNotes: 1\nUpsideDown: 1\nJudgementLine: 1\nBarlineHeight: oops\n",
            7,
        );
        assert_eq!(m.light_position, 400.0);
        assert_eq!(m.width_for_note_height_scale, Some(42.0));
        assert_eq!(m.barline_height, 2.5);
        assert!(m.keys_under_notes);
        assert!(m.upside_down);
        assert!(m.judgement_line);
    }

    #[test]
    fn named_colours_and_alpha() {
        let m = mania(
            "[Mania]\nKeys: 4\nColourColumnLine: 255,255,255,0\nColourJudgementLine: 1,2,3\n\
             ColourBarline: 4,5,6,7\nColourHold: 255,191,51,255\nColour2: 1,2\nColour3: 300,0,0\n",
            4,
        );
        assert_eq!(m.colours.column_line, rgba(255, 255, 255, 0));
        assert_eq!(
            m.colours.column_line.map(Rgba::disallow_zero_alpha),
            rgba(255, 255, 255, 255)
        );
        assert_eq!(m.colours.judgement_line, rgba(1, 2, 3, 255));
        assert_eq!(m.colours.barline, rgba(4, 5, 6, 7));
        assert_eq!(m.colours.hold, rgba(255, 191, 51, 255));
        assert_eq!(m.colours.column, vec![None; 4]);
        assert_eq!(
            rgba(1, 2, 3, 9).map(Rgba::disallow_zero_alpha),
            rgba(1, 2, 3, 9)
        );
    }

    #[test]
    fn keys_are_case_sensitive() {
        let m = mania("[Mania]\nKeys: 4\nhitposition: 300\nNOTEIMAGE0: x\n", 4);
        assert_eq!(m.hit_position, 402.0);
        assert_eq!(m.columns[0].note, None);
    }

    #[test]
    fn missing_block_gets_lazer_defaults() {
        let params = SkinIniParams::default();
        let ini = parse_skin_ini(b"[General]\nVersion: 2.5\n[Mania]\nKeys: 4\n");
        assert!(ini.mania(7).is_none());
        let m = ini.mania_or_default(7, &params);
        assert_eq!(m, ManiaConfig::defaults(7, 2.5, &params));
        assert_eq!(m.keys, 7);
        assert_eq!(m.column_width, vec![30.0; 7]);
        assert_eq!(m.column_spacing, vec![0.0; 6]);
        assert_eq!(m.column_line_width, vec![2.0; 8]);
        assert_eq!(m.hit_position, 402.0);
        assert_eq!(m.light_position, 413.0);
        assert_eq!(m.barline_height, 1.2);
        assert!(m.judgement_line);
        assert_eq!(m.note_body_style, NoteBodyStyle::RepeatBottom);
        assert_eq!(m.score_position, 300.0);
        assert_eq!(m.combo_position, 111.0);
        assert_eq!(m.lighting_n_width, vec![0.0; 7]);
        assert_eq!(m.lighting_l_width, vec![0.0; 7]);
        assert_eq!(m.colours.light, vec![None; 7]);
        assert_eq!(m.hits, HitImages::default());
    }

    #[test]
    fn effect_positions_widths_and_light_colours() {
        let m = mania(
            "[Mania]\nKeys: 4\nScorePosition: 250\nComboPosition: 140\nLightingNWidth: 40,oops,42\n\
             LightingLWidth: 50,50,50,50,99\nColourLight1: 1,2,3\nColourLight4: 4,5,6,0\n\
             ColourLight5: 7,8,9\nColourLight01: 9,9,9\nComboPosition: nope\n",
            4,
        );
        assert_eq!(m.score_position, 250.0);
        assert_eq!(
            m.combo_position, 140.0,
            "an unparsable value keeps the last"
        );
        assert_eq!(m.lighting_n_width, vec![40.0, 0.0, 42.0, 0.0]);
        assert_eq!(m.lighting_l_width, vec![50.0; 4]);
        assert_eq!(
            m.colours.light,
            vec![rgba(1, 2, 3, 255), None, None, rgba(4, 5, 6, 0)]
        );
        assert_eq!(
            m.colours.column,
            vec![None; 4],
            "ColourLight# is not Colour#"
        );
    }

    #[test]
    fn hit_burst_images() {
        let m = mania(
            "[Mania]\nKeys: 7\nHit0: j\\miss\nHit50: j/50\nHit100: h100\nHit200: h200\nHit300: h300\n\
             Hit300g: j\\max\nHit301: nope\n",
            7,
        );
        assert_eq!(
            m.hits,
            HitImages {
                h0: Some("j/miss".into()),
                h50: Some("j/50".into()),
                h100: Some("h100".into()),
                h200: Some("h200".into()),
                h300: Some("h300".into()),
                h300g: Some("j/max".into()),
            }
        );
        assert_eq!(m.columns[0], ColumnImages::default());
    }

    #[test]
    fn fonts_section_combo_prefix_and_overlap() {
        let ini = parse_skin_ini(
            b"[Fonts]\r\nScorePrefix: s\r\nComboPrefix: Fonts\\combo\r\nComboOverlap: 3\r\n[General]\r\nComboPrefix: wrong\r\n",
        );
        assert_eq!(ini.fonts.combo_prefix.as_deref(), Some("Fonts/combo"));
        assert_eq!(ini.fonts.combo_overlap, Some(3.0));

        let ini = parse_skin_ini(b"[fonts]\nComboOverlap: -2.5\nComboPrefix:\n");
        assert_eq!(ini.fonts.combo_overlap, Some(-2.5));
        assert_eq!(ini.fonts.combo_prefix, None, "an empty prefix is unset");
        assert_eq!(parse_skin_ini(b"").fonts, SkinFonts::default());
        assert_eq!(
            SkinIni::absent(&SkinIniParams::default()).fonts,
            SkinFonts::default()
        );
    }

    proptest! {
        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = parse_skin_ini(&bytes);
        }

        #[test]
        fn arbitrary_mania_lines_never_panic(lines in proptest::collection::vec("[ -~]{0,40}", 0..20)) {
            let text = format!("[Mania]\nKeys: 7\n{}", lines.join("\n"));
            let _ = parse_skin_ini(text.as_bytes());
        }
    }
}
