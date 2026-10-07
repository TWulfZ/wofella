//! Read-only access to `Skins/<folder>/`: the skin list, and the images a mania playfield draws,
//! resolved the way lazer imitates stable (MIT, ppy/osu @ 6359741babba4ced42da1fcda738c35370fdf3f4;
//! stable's source is closed):
//!
//! - column type `1 2 1 S 1 2 1` for 7K: `LegacyManiaColumnElement.cs` L31-41,
//!   `StageDefinition.cs` L32;
//! - chains (an explicit `skin.ini` reference replaces the default name, it does not fall back to
//!   it): note `LegacyNotePiece.cs` L84-103, head `LegacyHoldNoteHeadPiece.cs` L49-50, tail
//!   `LegacyHoldNoteTailPiece.cs` L58-60, body `LegacyBodyPiece.cs` L45-46, keys
//!   `LegacyKeyArea.cs` L38-42, stage `LegacyStageBackground.cs` L33-37,
//!   `LegacyStageForeground.cs` L28-29, `LegacyHitTarget.cs` L26-27, `LegacyColumnBackground.cs`
//!   L32-33;
//! - names: `@2x` stripped then tried first (`LegacySkin.cs` L569-578), extensions none, `.png`,
//!   `.jpg` (osu-framework `TextureLoaderStore`), `\` as `/` and case-insensitive files
//!   (`RealmBackedResourceStore.cs` L52-59), frame `-0` before the plain name for animated
//!   elements only (`LegacySkinExtensions.cs` L69, the `FindProvider` order);
//! - no `skin.ini` means the latest version (`Skin.cs` L110-117);
//! - playback effects: hit bursts `ManiaLegacySkinTransformer.cs` L33-57 (names) and L204-207
//!   (explicit name or default, animatable), lighting `LegacyHitExplosion.cs` L33-34 and
//!   `LegacyBodyPiece.cs` L48-49 (`lightingN`, `lightingL`), frames read until the first gap
//!   (`LegacySkinExtensions.cs` L93-107), combo digits `{prefix}-{digit}`
//!   (`LegacySpriteText.cs` L93-95) with the prefix `score` by default
//!   (`LegacySkinExtensions.cs` L150-151).
//!
//! Deliberate deviations: only frame 0 is read, except for lighting, whose frames are capped; a
//! playback-effect image that would take the skin past its byte cap is dropped with a diagnostic
//! instead of failing the load, so effects never cost a skin its playfield; an effect whose
//! explicit reference resolves nothing falls back to its default name in the same skin, where
//! lazer would fall back to its own default skin, which cannot ship (ADR 0019); and a file that fails the magic, header or cap
//! checks counts as absent so the chain moves on, as lazer does for a texture it cannot load. Containment is
//! textual (ADR 0018): references are split into single entries matched against directory
//! listings, and symlinks and junctions are followed on purpose.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use wolluf_core::ErrorCode;

use crate::codec::skin_ini::{
    ManiaConfig, SkinIni, SkinIniParams, StageImages, parse_skin_ini_with,
};
use crate::songs::{SongFileError, is_single_entry_name, read_capped};
use crate::stable::stable_str_enum;

/// Caps for files anyone can drop into `Skins/`. Pilot install (2026-10-06): 27 skins, at most
/// 1,445 files per skin, `skin.ini` at most 16.7 KB, mania images at most 323 KB, the active
/// skin's LN body 91,439 bytes at 138×40000 px, and 647 KB for the largest loaded 7K skin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinsParams {
    pub max_skins: usize,
    pub max_dir_entries: usize,
    /// Folders plus file name; the pilot's deepest reference is 2.
    pub max_ref_depth: usize,
    pub max_ini_bytes: u64,
    pub max_image_bytes: u64,
    /// All distinct files of one loaded skin; above it the load fails instead of half-skinning.
    pub max_skin_bytes: u64,
    /// A small PNG can declare a huge bitmap; 138×40000 LN bodies (5.5 Mpx) must pass.
    pub max_image_pixels: u64,
    /// Decoded size of one skin's distinct files: what the webview's `ImageBitmap`s will hold.
    pub max_total_pixels: u64,
    /// lazer reads lighting frames until the first gap with no limit; a cap keeps a stray
    /// `-0` … `-999` sequence from filling the payload.
    pub max_effect_frames: usize,
    pub ini: SkinIniParams,
}

impl Default for SkinsParams {
    fn default() -> Self {
        Self {
            max_skins: 1_024,
            max_dir_entries: 8_192,
            max_ref_depth: 4,
            max_ini_bytes: 256 * 1_024,
            max_image_bytes: 2 * 1_024 * 1_024,
            max_skin_bytes: 8 * 1_024 * 1_024,
            max_image_pixels: 8 * 1_024 * 1_024,
            max_total_pixels: 64 * 1_024 * 1_024,
            max_effect_frames: 60,
            ini: SkinIniParams::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SkinError {
    #[error("skin folder is missing")]
    Missing,
    #[error("keymode {keys} is not supported")]
    InvalidKeymode { keys: u8 },
    #[error("skin data is {size} bytes, above the {max} byte cap")]
    TooLarge { size: u64, max: u64 },
    #[error("skin read failed: {kind:?}")]
    Io { kind: io::ErrorKind },
}

impl SkinError {
    /// `TooLarge` is a limit of wolluf, not a request the caller got wrong.
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Missing => ErrorCode::NotFound,
            Self::InvalidKeymode { .. } => ErrorCode::InvalidInput,
            Self::TooLarge { .. } => ErrorCode::UnsupportedFormat,
            Self::Io { .. } => ErrorCode::Internal,
        }
    }
}

stable_str_enum! {
    /// Append-only and never renumbered.
    pub enum SkinDiagCode {
        IniMissing => "skin.ini_missing",
        IniLossy => "skin.ini_lossy",
        KeysBlockMissing => "skin.keys_block_missing",
        ListingTruncated => "skin.listing_truncated",
        RefRejected => "skin.ref_rejected",
        ImageMissing => "skin.image_missing",
        ImageUnreadable => "skin.image_unreadable",
        ImageTooLarge => "skin.image_too_large",
        ImageBadFormat => "skin.image_bad_format",
        ImageBadHeader => "skin.image_bad_header",
        ImageTooManyPixels => "skin.image_too_many_pixels",
        PixelBudgetExceeded => "skin.pixel_budget_exceeded",
        EffectBudgetExceeded => "skin.effect_budget_exceeded",
    }
}

/// Carries the slot only, never a reference or file name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinDiagnostic {
    pub code: SkinDiagCode,
    pub slot: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinEntry {
    pub folder: String,
    pub has_ini: bool,
    pub name: Option<String>,
    pub keys: Vec<u8>,
    /// Nanoseconds since the Unix epoch, as text (no 64-bit ints over IPC, ADR 0009): the UI's
    /// skin cache key, so an edited `skin.ini` reloads (ADR 0019).
    pub ini_mtime: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
}

impl ImageKind {
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinFile {
    /// Relative to the skin folder, `/`-separated, with the on-disk spelling.
    pub path: String,
    pub kind: ImageKind,
    /// 2 for an `@2x` file: its display size is the pixel size halved.
    pub scale: u8,
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

/// `slot` ids are stable across IPC: `note.{i}`, `note.{i}.head`, `note.{i}.tail`, `body.{i}`,
/// `key.{i}`, `key.{i}.down`, `stage.{left,right,bottom,hint,light}`, with 0-based columns, and
/// for playback effects `hit.{0,50,100,200,300,300g}`, `combo.{0-9}` and
/// `lighting.{n,l}.{frame}` with 0-based frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinImage {
    pub slot: String,
    /// Index into `LoadedSkin::files`, shared by every slot that resolves to the same file.
    pub file: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedSkin {
    pub folder: String,
    pub name: Option<String>,
    pub version: f64,
    pub config: ManiaConfig,
    pub fonts: FontConfig,
    /// Resolved slots only; a slot that is absent here is drawn procedurally.
    pub images: Vec<SkinImage>,
    pub files: Vec<SkinFile>,
    pub diagnostics: Vec<SkinDiagnostic>,
}

/// `[Fonts]` with lazer's defaults applied (`LegacySkinExtensions.cs` L176-177).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontConfig {
    /// Pixels the combo digits overlap; negative adds a gap.
    pub combo_overlap: f32,
}

const SKIN_INI: &str = "skin.ini";
/// Probed by path so that a truncated listing cannot hide the ini; NTFS matches any case.
const SKIN_INI_SPELLINGS: [&str; 2] = [SKIN_INI, "Skin.ini"];
const SEPARATOR: char = '/';
const HIGH_RES: &str = "@2x";
const HIGH_RES_SCALE: u8 = 2;
const FIRST_FRAME: &str = "-0";
const EXTENSIONS: [&str; 3] = ["", ".png", ".jpg"];
const NOTE: &str = "mania-note";
const KEY: &str = "mania-key";
const STAGE: &str = "mania-stage-";
const HIT: &str = "mania-hit";
const COMBO_PREFIX: &str = "score";
const LIGHTING_N: &str = "lightingN";
const LIGHTING_L: &str = "lightingL";
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const PNG_IHDR: &[u8] = b"IHDR";
const PNG_IHDR_LEN: u32 = 13;
/// PNG dimensions are 31-bit (PNG spec §11.2.2).
const PNG_MAX_SIDE: u32 = 0x7fff_ffff;
const JPEG_SOI: &[u8] = &[0xff, 0xd8];
/// Lf, P, Y, X and Nf (ITU T.81 §B.2.2): a shorter segment would put Y and X outside itself.
const JPEG_SOF_MIN_LEN: usize = 8;

/// `Skins/` entries that are folders with a usable name, sorted by folder. One unreadable skin
/// lists without name or keys rather than hiding the others.
pub fn list_skins(skins_dir: &Path, params: &SkinsParams) -> Result<Vec<SkinEntry>, SkinError> {
    let entries = fs::read_dir(skins_dir).map_err(dir_error)?;
    let mut out = Vec::new();
    for entry in entries.take(params.max_skins) {
        let Ok(entry) = entry else { continue };
        let Ok(folder) = entry.file_name().into_string() else {
            continue;
        };
        let dir = entry.path();
        if is_single_entry_name(&folder) && dir.is_dir() {
            out.push(describe(&dir, folder, params));
        }
    }
    out.sort_by(|a, b| a.folder.cmp(&b.folder));
    Ok(out)
}

fn describe(dir: &Path, folder: String, params: &SkinsParams) -> SkinEntry {
    let ini_path = list_dir(dir, params.max_dir_entries)
        .ok()
        .and_then(|(entries, _)| find_ini(dir, &entries));
    let ini = ini_path
        .as_deref()
        .and_then(|p| read_capped(p, params.max_ini_bytes).ok())
        .map(|bytes| parse_skin_ini_with(&bytes, &params.ini));
    let ini_mtime = ini_path
        .as_deref()
        .and_then(|p| fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().to_string());
    SkinEntry {
        folder,
        ini_mtime,
        has_ini: ini_path.is_some(),
        name: ini.as_ref().and_then(|i| i.name.clone()),
        keys: ini
            .map(|i| i.mania.keys().copied().collect())
            .unwrap_or_default(),
    }
}

/// `folder` comes from the webview, so it must be one entry of `skins_dir`.
pub fn load_skin(
    skins_dir: &Path,
    folder: &str,
    keymode: u8,
    params: &SkinsParams,
) -> Result<LoadedSkin, SkinError> {
    if keymode == 0 || keymode > params.ini.max_keys {
        return Err(SkinError::InvalidKeymode { keys: keymode });
    }
    if !is_single_entry_name(folder) {
        return Err(SkinError::Missing);
    }
    let folder = listed_folder(skins_dir, folder, params.max_skins)?;
    let root = skins_dir.join(&folder);
    if !root.is_dir() {
        return Err(SkinError::Missing);
    }
    let (top, truncated) = list_dir(&root, params.max_dir_entries).map_err(dir_error)?;
    let mut resolver = Resolver::new(root, params);
    if truncated {
        resolver.diag(SkinDiagCode::ListingTruncated, None);
    }
    let ini_path = find_ini(&resolver.root, &top);
    resolver.listings.insert(String::new(), top);
    let ini = match ini_path.map(|p| read_capped(&p, params.max_ini_bytes)) {
        Some(Ok(bytes)) => parse_skin_ini_with(&bytes, &params.ini),
        Some(Err(SongFileError::TooLarge { size, max })) => {
            return Err(SkinError::TooLarge { size, max });
        }
        Some(Err(SongFileError::Io { kind })) => return Err(SkinError::Io { kind }),
        Some(Err(SongFileError::Missing)) | None => {
            resolver.diag(SkinDiagCode::IniMissing, None);
            SkinIni::absent(&params.ini)
        }
    };
    if ini.decoded_lossily {
        resolver.diag(SkinDiagCode::IniLossy, None);
    }
    if ini.mania(keymode).is_none() {
        resolver.diag(SkinDiagCode::KeysBlockMissing, None);
    }
    let config = ini.mania_or_default(keymode, &params.ini);
    let mut images = Vec::new();
    for slot in slots(&config) {
        resolver.resolve_slot(slot, &mut images)?;
    }
    // Resolved last so the playfield's own images claim the byte budget first.
    resolver.soft_budget = true;
    for slot in effect_slots(&config, ini.fonts.combo_prefix.as_deref()) {
        resolver.resolve_slot(slot, &mut images)?;
    }
    Ok(LoadedSkin {
        folder,
        name: ini.name,
        version: ini.version,
        fonts: FontConfig {
            combo_overlap: ini.fonts.combo_overlap.unwrap_or(0.0),
        },
        config,
        images,
        files: resolver.files,
        diagnostics: resolver.diagnostics,
    })
}

/// NTFS and Win32 open `S`, `s.` and `s ` as `s`, so only a byte-for-byte match with an entry
/// that `list_skins` can show names a skin; the same cap keeps both views of `Skins/` equal.
fn listed_folder(skins_dir: &Path, folder: &str, cap: usize) -> Result<String, SkinError> {
    fs::read_dir(skins_dir)
        .map_err(dir_error)?
        .take(cap)
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .find(|name| name == folder)
        .ok_or(SkinError::Missing)
}

/// Windows refuses to list or open some names (`InvalidInput`, `InvalidFilename`) and answers
/// `PermissionDenied` for a file opened as a directory; none of them is a skin.
fn dir_error(e: io::Error) -> SkinError {
    match e.kind() {
        io::ErrorKind::NotFound
        | io::ErrorKind::NotADirectory
        | io::ErrorKind::InvalidInput
        | io::ErrorKind::InvalidFilename
        | io::ErrorKind::PermissionDenied => SkinError::Missing,
        kind => SkinError::Io { kind },
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    name: String,
    lower: String,
}

/// Sorted so that resolution does not depend on the file system's listing order.
pub(crate) fn list_dir(dir: &Path, cap: usize) -> io::Result<(Vec<Entry>, bool)> {
    let mut entries = Vec::new();
    let mut truncated = false;
    for (n, entry) in fs::read_dir(dir)?.enumerate() {
        if n == cap {
            truncated = true;
            break;
        }
        let Ok(name) = entry.map(|e| e.file_name()) else {
            continue;
        };
        if let Ok(name) = name.into_string() {
            let lower = name.to_lowercase();
            entries.push(Entry { name, lower });
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((entries, truncated))
}

/// File names compare case-insensitively as on NTFS; on a case-sensitive file system the exact
/// spelling wins over other spellings.
pub(crate) fn matching<'a>(entries: &'a [Entry], wanted: &str) -> Vec<&'a str> {
    let lower = wanted.to_lowercase();
    let exact = entries.iter().filter(|e| e.name == wanted);
    let folded = entries
        .iter()
        .filter(|e| e.lower == lower && e.name != wanted);
    exact.chain(folded).map(|e| e.name.as_str()).collect()
}

/// stable and the pilot's skins spell it both `skin.ini` and `Skin.ini`.
fn find_ini(dir: &Path, entries: &[Entry]) -> Option<PathBuf> {
    matching(entries, SKIN_INI)
        .into_iter()
        .chain(SKIN_INI_SPELLINGS)
        .map(|name| dir.join(name))
        .find(|p| p.is_file())
}

struct Slot {
    key: String,
    /// References tried in order; each is an explicit `skin.ini` value or a default name.
    chain: Vec<String>,
    animated: bool,
    /// Every frame is resolved, as `{key}.{frame}`; otherwise frame 0 only, as `key`.
    frames: bool,
}

fn column_type(column: usize, keys: usize) -> &'static str {
    if !keys.is_multiple_of(2) && column == keys / 2 {
        "S"
    } else if column.min(keys - 1 - column).is_multiple_of(2) {
        "1"
    } else {
        "2"
    }
}

fn slots(config: &ManiaConfig) -> Vec<Slot> {
    let keys = config.columns.len();
    let slot = |key: String, chain: Vec<String>, animated: bool| Slot {
        key,
        chain,
        animated,
        frames: false,
    };
    let pick = |explicit: &Option<String>, default: String| explicit.clone().unwrap_or(default);
    let mut out = Vec::new();
    for (i, column) in config.columns.iter().enumerate() {
        let t = column_type(i, keys);
        let note = pick(&column.note, format!("{NOTE}{t}"));
        let head = pick(&column.head, format!("{NOTE}{t}H"));
        let tail = pick(&column.tail, format!("{NOTE}{t}T"));
        let body = pick(&column.body, format!("{NOTE}{t}L"));
        let key = pick(&column.key, format!("{KEY}{t}"));
        let key_down = pick(&column.key_down, format!("{KEY}{t}D"));
        out.push(slot(format!("note.{i}"), vec![note.clone()], true));
        out.push(slot(
            format!("note.{i}.head"),
            vec![head.clone(), note.clone()],
            true,
        ));
        out.push(slot(format!("note.{i}.tail"), vec![tail, head, note], true));
        out.push(slot(format!("body.{i}"), vec![body], true));
        out.push(slot(format!("key.{i}"), vec![key], false));
        out.push(slot(format!("key.{i}.down"), vec![key_down], false));
    }
    let StageImages {
        left,
        right,
        bottom,
        hint,
        light,
        ..
    } = &config.stage;
    for (name, explicit, animated) in [
        ("left", left, false),
        ("right", right, false),
        ("bottom", bottom, true),
        ("hint", hint, false),
        ("light", light, true),
    ] {
        let reference = pick(explicit, format!("{STAGE}{name}"));
        out.push(slot(format!("stage.{name}"), vec![reference], animated));
    }
    out
}

/// The explicit reference, then the default name: the pilot's active skin names `mania/stage/…`
/// paths that do not exist while the files sit in its root.
fn explicit_then_default(explicit: &Option<String>, default: String) -> Vec<String> {
    explicit
        .iter()
        .cloned()
        .chain((explicit.as_ref() != Some(&default)).then_some(default))
        .collect()
}

/// What the Label preview's autoplay effects draw: judgement bursts, combo digits and lighting.
fn effect_slots(config: &ManiaConfig, combo_prefix: Option<&str>) -> Vec<Slot> {
    let hits = &config.hits;
    let mut out: Vec<Slot> = [
        ("0", &hits.h0),
        ("50", &hits.h50),
        ("100", &hits.h100),
        ("200", &hits.h200),
        ("300", &hits.h300),
        ("300g", &hits.h300g),
    ]
    .into_iter()
    .map(|(result, explicit)| Slot {
        key: format!("hit.{result}"),
        chain: explicit_then_default(explicit, format!("{HIT}{result}")),
        animated: true,
        frames: false,
    })
    .collect();
    let prefix = combo_prefix.unwrap_or(COMBO_PREFIX);
    out.extend((0..10).map(|digit| Slot {
        key: format!("combo.{digit}"),
        chain: vec![format!("{prefix}-{digit}")],
        animated: false,
        frames: false,
    }));
    for (kind, explicit, default) in [
        ("n", &config.stage.lighting_n, LIGHTING_N),
        ("l", &config.stage.lighting_l, LIGHTING_L),
    ] {
        out.push(Slot {
            key: format!("lighting.{kind}"),
            chain: explicit_then_default(explicit, default.to_owned()),
            animated: true,
            frames: true,
        });
    }
    out
}

/// lazer inserts `@2x` before the extension of the last segment (`Path.ChangeExtension`).
fn high_res(name: &str) -> String {
    match name.rfind('.') {
        Some(dot) if dot + 1 < name.len() => {
            format!("{}{HIGH_RES}{}", &name[..dot], &name[dot..])
        }
        _ => format!("{name}{HIGH_RES}"),
    }
}

type Outcome = Result<usize, SkinDiagCode>;

struct Resolver<'p> {
    root: PathBuf,
    params: &'p SkinsParams,
    /// Keyed by the `/`-joined on-disk relative directory; `""` is the skin folder.
    listings: BTreeMap<String, Vec<Entry>>,
    loaded: BTreeMap<(String, u8), Outcome>,
    files: Vec<SkinFile>,
    total_bytes: u64,
    total_pixels: u64,
    /// Past the byte cap, an image becomes a missing slot instead of failing the load.
    soft_budget: bool,
    diagnostics: Vec<SkinDiagnostic>,
}

impl<'p> Resolver<'p> {
    fn new(root: PathBuf, params: &'p SkinsParams) -> Self {
        Self {
            root,
            params,
            listings: BTreeMap::new(),
            loaded: BTreeMap::new(),
            files: Vec::new(),
            total_bytes: 0,
            total_pixels: 0,
            soft_budget: false,
            diagnostics: Vec::new(),
        }
    }

    fn resolve_slot(&mut self, slot: Slot, images: &mut Vec<SkinImage>) -> Result<(), SkinError> {
        if slot.frames {
            let mut frames = Vec::new();
            for reference in &slot.chain {
                frames = self.resolve_frames(&slot.key, reference)?;
                if !frames.is_empty() {
                    break;
                }
            }
            if frames.is_empty() {
                self.diag(SkinDiagCode::ImageMissing, Some(&format!("{}.0", slot.key)));
            }
            for (frame, file) in frames.into_iter().enumerate() {
                images.push(SkinImage {
                    slot: format!("{}.{frame}", slot.key),
                    file,
                });
            }
            return Ok(());
        }
        let mut found = None;
        for reference in &slot.chain {
            found = self.resolve(&slot.key, reference, slot.animated)?;
            if found.is_some() {
                break;
            }
        }
        match found {
            Some(file) => images.push(SkinImage {
                slot: slot.key,
                file,
            }),
            None => self.diag(SkinDiagCode::ImageMissing, Some(&slot.key)),
        }
        Ok(())
    }

    fn diag(&mut self, code: SkinDiagCode, slot: Option<&str>) {
        let diagnostic = SkinDiagnostic {
            code,
            slot: slot.map(str::to_owned),
        };
        if !self.diagnostics.contains(&diagnostic) {
            self.diagnostics.push(diagnostic);
        }
    }

    fn path_of(&self, rel: &str) -> PathBuf {
        rel.split(SEPARATOR)
            .filter(|c| !c.is_empty())
            .fold(self.root.clone(), |p, c| p.join(c))
    }

    /// Relative paths of the entries of `dir` spelled like `wanted`, folders or files as asked.
    fn find(&mut self, dir: &str, wanted: &str, want_dir: bool) -> Vec<String> {
        if !self.listings.contains_key(dir) {
            let entries = match list_dir(&self.path_of(dir), self.params.max_dir_entries) {
                Ok((entries, truncated)) => {
                    if truncated {
                        self.diag(SkinDiagCode::ListingTruncated, None);
                    }
                    entries
                }
                Err(_) => Vec::new(),
            };
            self.listings.insert(dir.to_owned(), entries);
        }
        let candidates: Vec<String> = self
            .listings
            .get(dir)
            .map(|entries| {
                matching(entries, wanted)
                    .into_iter()
                    .map(|name| {
                        if dir.is_empty() {
                            name.to_owned()
                        } else {
                            format!("{dir}{SEPARATOR}{name}")
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        candidates
            .into_iter()
            .filter(|rel| {
                let path = self.path_of(rel);
                if want_dir {
                    path.is_dir()
                } else {
                    path.is_file()
                }
            })
            .collect()
    }

    /// The folder (`/`-joined, on-disk spelling) and last component of `reference`, or `None`
    /// when it is blank, rejected or names a folder that does not exist.
    fn locate(&mut self, slot: &str, reference: &str) -> Option<(String, String)> {
        let normalized = reference.replace('\\', "/").replace(HIGH_RES, "");
        if normalized.trim().is_empty() {
            return None;
        }
        let parts: Vec<&str> = normalized.split(SEPARATOR).collect();
        // `:` is a drive prefix or an alternate data stream on Windows; refusing it everywhere
        // keeps one skin resolving the same way in Linux CI and on the pilot's PC.
        let entries_only = parts
            .iter()
            .all(|p| is_single_entry_name(p) && !p.contains(':'));
        let (&last, dirs) = parts.split_last()?;
        if !entries_only || parts.len() > self.params.max_ref_depth {
            self.diag(SkinDiagCode::RefRejected, Some(slot));
            return None;
        }
        let mut dir = String::new();
        for name in dirs {
            dir = self.find(&dir, name, true).into_iter().next()?;
        }
        Some((dir, last.to_owned()))
    }

    /// `stem@2x` (scale 2) before `stem`, each as given, `.png` and `.jpg`.
    fn image(&mut self, slot: &str, dir: &str, stem: &str) -> Result<Option<usize>, SkinError> {
        for (scale, name) in [(HIGH_RES_SCALE, high_res(stem)), (1, stem.to_owned())] {
            for ext in EXTENSIONS {
                for rel in self.find(dir, &format!("{name}{ext}"), false) {
                    if let Some(file) = self.load(slot, rel, scale)? {
                        return Ok(Some(file));
                    }
                }
            }
        }
        Ok(None)
    }

    fn resolve(
        &mut self,
        slot: &str,
        reference: &str,
        animated: bool,
    ) -> Result<Option<usize>, SkinError> {
        let Some((dir, last)) = self.locate(slot, reference) else {
            return Ok(None);
        };
        if animated && let Some(file) = self.image(slot, &dir, &format!("{last}{FIRST_FRAME}"))? {
            return Ok(Some(file));
        }
        self.image(slot, &dir, &last)
    }

    /// `name-0`, `name-1`, … until the first gap, or `name` alone as a single frame.
    fn resolve_frames(&mut self, key: &str, reference: &str) -> Result<Vec<usize>, SkinError> {
        let first = format!("{key}.0");
        let Some((dir, last)) = self.locate(&first, reference) else {
            return Ok(Vec::new());
        };
        let mut frames = Vec::new();
        while frames.len() < self.params.max_effect_frames {
            let slot = format!("{key}.{}", frames.len());
            match self.image(&slot, &dir, &format!("{last}-{}", frames.len()))? {
                Some(file) => frames.push(file),
                None => break,
            }
        }
        if frames.is_empty()
            && self.params.max_effect_frames > 0
            && let Some(file) = self.image(&first, &dir, &last)?
        {
            frames.push(file);
        }
        Ok(frames)
    }

    fn load(&mut self, slot: &str, rel: String, scale: u8) -> Result<Option<usize>, SkinError> {
        let outcome = match self.loaded.get(&(rel.clone(), scale)) {
            Some(outcome) => *outcome,
            None => {
                let outcome = self.read_image(&rel, scale)?;
                self.loaded.insert((rel, scale), outcome);
                outcome
            }
        };
        match outcome {
            Ok(file) => Ok(Some(file)),
            Err(code) => {
                self.diag(code, Some(slot));
                Ok(None)
            }
        }
    }

    fn read_image(&mut self, rel: &str, scale: u8) -> Result<Outcome, SkinError> {
        let bytes = match read_capped(&self.path_of(rel), self.params.max_image_bytes) {
            Ok(bytes) => bytes,
            Err(SongFileError::TooLarge { .. }) => return Ok(Err(SkinDiagCode::ImageTooLarge)),
            Err(SongFileError::Missing | SongFileError::Io { .. }) => {
                return Ok(Err(SkinDiagCode::ImageUnreadable));
            }
        };
        let (kind, width, height) = match sniff(&bytes) {
            Ok(header) => header,
            Err(code) => return Ok(Err(code)),
        };
        let pixels = u64::from(width) * u64::from(height);
        if pixels > self.params.max_image_pixels {
            return Ok(Err(SkinDiagCode::ImageTooManyPixels));
        }
        let total_pixels = self.total_pixels.saturating_add(pixels);
        if total_pixels > self.params.max_total_pixels {
            return Ok(Err(SkinDiagCode::PixelBudgetExceeded));
        }
        let total = self.total_bytes.saturating_add(bytes.len() as u64);
        if total > self.params.max_skin_bytes && self.soft_budget {
            return Ok(Err(SkinDiagCode::EffectBudgetExceeded));
        }
        if total > self.params.max_skin_bytes {
            return Err(SkinError::TooLarge {
                size: total,
                max: self.params.max_skin_bytes,
            });
        }
        self.total_bytes = total;
        self.total_pixels = total_pixels;
        self.files.push(SkinFile {
            path: rel.to_owned(),
            kind,
            scale,
            width,
            height,
            bytes,
        });
        Ok(Ok(self.files.len() - 1))
    }
}

/// The webview decodes only PNG and JPEG here, while stable also loads other formats saved under
/// a `.png` name (the pilot has 22.7 MB TIFF LN bodies), so the bytes decide, not the extension.
pub(crate) fn sniff(bytes: &[u8]) -> Result<(ImageKind, u32, u32), SkinDiagCode> {
    let (kind, size) = if bytes.starts_with(PNG_SIGNATURE) {
        (ImageKind::Png, png_size(bytes))
    } else if bytes.starts_with(JPEG_SOI) {
        (ImageKind::Jpeg, jpeg_size(bytes))
    } else {
        return Err(SkinDiagCode::ImageBadFormat);
    };
    size.map(|(w, h)| (kind, w, h))
        .ok_or(SkinDiagCode::ImageBadHeader)
}

fn be_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn be_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// IHDR must be the first chunk (PNG spec §5.6).
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let at = PNG_SIGNATURE.len();
    if be_u32(bytes, at)? != PNG_IHDR_LEN || bytes.get(at + 4..at + 8)? != PNG_IHDR {
        return None;
    }
    let side = 1..=PNG_MAX_SIDE;
    let width = be_u32(bytes, at + 8).filter(|w| side.contains(w))?;
    let height = be_u32(bytes, at + 12).filter(|h| side.contains(h))?;
    Some((width, height))
}

/// Walks the marker segments up to the frame header (ITU T.81 §B.1.1). Only Huffman SOF0-SOF2
/// frames are accepted, since browsers do not decode the lossless and arithmetic ones; a height
/// of 0 defers to a DNL marker, which they do not support either.
fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut at = JPEG_SOI.len();
    loop {
        if *bytes.get(at)? != 0xff {
            return None;
        }
        while *bytes.get(at)? == 0xff {
            at += 1;
        }
        let marker = *bytes.get(at)?;
        at += 1;
        match marker {
            0x01 | 0xd0..=0xd7 => continue,
            0x00 | 0xd8..=0xda => return None,
            _ => {}
        }
        let len = usize::from(be_u16(bytes, at)?);
        if len < 2 {
            return None;
        }
        match marker {
            0xc0..=0xc2 => {
                if len < JPEG_SOF_MIN_LEN {
                    return None;
                }
                let height = be_u16(bytes, at + 3).filter(|&h| h > 0)?;
                let width = be_u16(bytes, at + 5).filter(|&w| w > 0)?;
                return Some((u32::from(width), u32::from(height)));
            }
            0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf => return None,
            _ => at += len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn column_types_follow_lazer() {
        let types = |keys: usize| (0..keys).map(|c| column_type(c, keys)).collect::<Vec<_>>();
        assert_eq!(types(7), ["1", "2", "1", "S", "1", "2", "1"]);
        assert_eq!(types(4), ["1", "2", "2", "1"]);
        assert_eq!(types(1), ["S"]);
    }

    #[test]
    fn high_res_goes_before_the_extension() {
        assert_eq!(high_res("mania-note1"), "mania-note1@2x");
        assert_eq!(high_res("blank.png"), "blank@2x.png");
        assert_eq!(high_res("7K.1.5"), "7K.1@2x.5");
        assert_eq!(high_res("odd."), "odd.@2x");
    }

    #[test]
    fn png_header_needs_ihdr_and_positive_sides() {
        let png = |w: u32, h: u32, chunk: &[u8]| {
            let mut out = PNG_SIGNATURE.to_vec();
            out.extend_from_slice(&13u32.to_be_bytes());
            out.extend_from_slice(chunk);
            out.extend_from_slice(&w.to_be_bytes());
            out.extend_from_slice(&h.to_be_bytes());
            out
        };
        assert_eq!(png_size(&png(270, 150, PNG_IHDR)), Some((270, 150)));
        assert_eq!(png_size(&png(270, 150, b"IDAT")), None);
        assert_eq!(png_size(&png(0, 150, PNG_IHDR)), None);
        assert_eq!(png_size(&png(1 << 31, 1, PNG_IHDR)), None);
        assert_eq!(png_size(&png(2, 2, PNG_IHDR)[..20]), None);
    }

    #[test]
    fn png_ihdr_chunk_must_declare_13_bytes() {
        let png = |len: u32| {
            let mut out = PNG_SIGNATURE.to_vec();
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(PNG_IHDR);
            out.extend_from_slice(&270u32.to_be_bytes());
            out.extend_from_slice(&150u32.to_be_bytes());
            out.extend_from_slice(&[8, 6, 0, 0, 0]);
            out
        };
        assert_eq!(png_size(&png(13)), Some((270, 150)));
        for len in [0, 8, 12, 14, u32::MAX] {
            assert_eq!(png_size(&png(len)), None, "{len}");
        }
    }

    #[test]
    fn jpeg_frame_header_shorter_than_8_bytes_is_rejected() {
        let sof = |len: u8| {
            vec![
                0xff, 0xd8, 0xff, 0xc0, 0x00, len, 8, 0x00, 0x96, 0x01, 0x0e, 1, 1, 0x11, 0,
            ]
        };
        assert_eq!(jpeg_size(&sof(0x0b)), Some((270, 150)));
        assert_eq!(jpeg_size(&sof(8)), Some((270, 150)));
        for len in [2, 5, 7] {
            assert_eq!(jpeg_size(&sof(len)), None, "{len}");
        }
    }

    #[test]
    fn jpeg_header_skips_segments_and_restart_markers() {
        let jpeg = [
            0xff, 0xd8, 0xff, 0xd0, 0xff, 0xe1, 0x00, 0x04, 0xaa, 0xbb, 0xff, 0xc2, 0x00, 0x0b, 8,
            0x00, 0x96, 0x01, 0x0e, 1, 1, 0x11, 0,
        ];
        assert_eq!(jpeg_size(&jpeg), Some((270, 150)));
        assert_eq!(jpeg_size(&jpeg[..16]), None);
        assert_eq!(jpeg_size(&[0xff, 0xd8, 0xff, 0xe0, 0x00, 0x01]), None);
        assert_eq!(jpeg_size(&[0xff, 0xd8, 0x00]), None);
    }

    proptest! {
        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
            let _ = sniff(&bytes);
            let mut png = PNG_SIGNATURE.to_vec();
            png.extend_from_slice(&bytes);
            let _ = sniff(&png);
            let mut jpeg = JPEG_SOI.to_vec();
            jpeg.extend_from_slice(&bytes);
            let _ = sniff(&jpeg);
        }
    }
}
