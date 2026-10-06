//! `SkinsService`: the skins feature's only entry point for shells (D12). Skins are read in
//! place from the catalog install's `Skins/`; the webview names a skin by folder, never by path
//! (ADR 0019).

use std::path::{Path, PathBuf};

use wolluf_core::ErrorCode;
use wolluf_source_osu::codec::skin_ini::{ManiaColours, ManiaConfig, NoteBodyStyle, Rgba};
use wolluf_source_osu::skins::{LoadedSkin, SkinError, list_skins, load_skin};

use super::SkinParams;
use super::dto::{
    ManiaColoursDto, ManiaConfigDto, NoteBodyStyleDto, RgbaDto, SkinDiagnosticDto, SkinDto,
    SkinEntryDto, SkinFileDto, SkinImageRefDto, SkinListDto,
};
use crate::base64;
use crate::context::{AppContext, blocking_join_error, catalog_install, newest_user_cfg};
use crate::errors::{AppError, keys};

const SKINS_DIR: &str = "Skins";

pub struct SkinsService<'a> {
    ctx: &'a AppContext,
    params: SkinParams,
}

impl<'a> SkinsService<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self {
            ctx,
            params: SkinParams::default(),
        }
    }

    pub fn with_params(mut self, params: SkinParams) -> Self {
        self.params = params;
        self
    }

    /// `None` until a sync has built a catalog: the install is the one the library reads.
    async fn install_root(&self) -> Result<Option<PathBuf>, AppError> {
        let (user, cache) = (self.ctx.user_db().clone(), self.ctx.cache_db().clone());
        let install = tokio::task::spawn_blocking(move || catalog_install(&user, &cache))
            .await
            .map_err(blocking_join_error)?;
        match install {
            Ok(install) => Ok(install.map(|i| i.root_path)),
            Err(e) if e.code == ErrorCode::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Empty without a catalog install or a `Skins/` folder, so the playfield stays procedural.
    pub async fn list(&self) -> Result<SkinListDto, AppError> {
        let Some(root) = self.install_root().await? else {
            return Ok(SkinListDto::default());
        };
        let params = self.params.read;
        tokio::task::spawn_blocking(move || {
            let cfg = newest_user_cfg(&root).unwrap_or_default();
            let skins = match list_skins(&skins_dir(&root), &params) {
                Ok(skins) => skins,
                Err(SkinError::Missing) => Vec::new(),
                Err(e) => return Err(AppError::internal(format!("skin list: {e}"))),
            };
            let current = cfg
                .skin
                .filter(|wanted| skins.iter().any(|s| s.folder == *wanted));
            Ok(SkinListDto {
                skins: skins
                    .into_iter()
                    .map(|s| SkinEntryDto {
                        folder: s.folder,
                        name: s.name,
                        keymodes: s.keys,
                        ini_mtime: s.ini_mtime,
                    })
                    .collect(),
                current,
                mania_speed: cfg.mania_speed,
                mania_speed_bpm_scale: cfg.mania_speed_bpm_scale,
            })
        })
        .await
        .map_err(blocking_join_error)?
    }

    pub async fn get(&self, folder: &str, keymode: u8) -> Result<SkinDto, AppError> {
        let Some(root) = self.install_root().await? else {
            return Err(skin_unavailable(folder));
        };
        let params = self.params.read;
        let folder = folder.to_owned();
        tokio::task::spawn_blocking(move || {
            load_skin(&skins_dir(&root), &folder, keymode, &params)
                .map_err(|e| skin_error(e, &folder))
                .and_then(skin_dto)
        })
        .await
        .map_err(blocking_join_error)?
    }
}

fn skins_dir(root: &Path) -> PathBuf {
    root.join(SKINS_DIR)
}

fn skin_unavailable(folder: &str) -> AppError {
    AppError::not_found()
        .with_key(keys::SKIN_UNAVAILABLE)
        .with_arg("folder", folder)
}

fn skin_error(e: SkinError, folder: &str) -> AppError {
    match e {
        SkinError::Missing => skin_unavailable(folder),
        SkinError::InvalidKeymode { keys } => {
            AppError::invalid_input().with_arg("keymode", keys.to_string())
        }
        SkinError::TooLarge { size, max } => AppError::new(e.code())
            .with_key(keys::SKIN_TOO_LARGE)
            .with_arg("folder", folder)
            .with_arg("bytes", size.to_string())
            .with_arg("maxBytes", max.to_string()),
        SkinError::Io { .. } => AppError::internal(format!("skin {folder:?}: {e}")),
    }
}

fn skin_dto(skin: LoadedSkin) -> Result<SkinDto, AppError> {
    let images = skin
        .images
        .into_iter()
        .map(|i| {
            let file = u16::try_from(i.file)
                .map_err(|_| AppError::internal(format!("file index {} of {}", i.file, i.slot)))?;
            Ok(SkinImageRefDto { slot: i.slot, file })
        })
        .collect::<Result<_, AppError>>()?;
    Ok(SkinDto {
        folder: skin.folder,
        name: skin.name,
        version: skin.version,
        config: config_dto(&skin.config),
        images,
        files: skin
            .files
            .into_iter()
            .map(|f| SkinFileDto {
                mime: f.kind.mime().to_owned(),
                scale: f.scale,
                width: f.width,
                height: f.height,
                base64: base64::encode(&f.bytes),
            })
            .collect(),
        diagnostics: skin
            .diagnostics
            .into_iter()
            .map(|d| SkinDiagnosticDto {
                code: d.code.as_str().to_owned(),
                slot: d.slot,
            })
            .collect(),
    })
}

fn config_dto(c: &ManiaConfig) -> ManiaConfigDto {
    ManiaConfigDto {
        keys: c.keys,
        column_width: c.column_width.clone(),
        column_spacing: c.column_spacing.clone(),
        column_line_width: c.column_line_width.clone(),
        hit_position: c.hit_position,
        light_position: c.light_position,
        width_for_note_height_scale: note_height_scale(c),
        note_body_style: match c.note_body_style {
            NoteBodyStyle::Stretch => NoteBodyStyleDto::Stretch,
            NoteBodyStyle::RepeatTop => NoteBodyStyleDto::RepeatTop,
            NoteBodyStyle::RepeatBottom => NoteBodyStyleDto::RepeatBottom,
            NoteBodyStyle::RepeatTopAndBottom => NoteBodyStyleDto::RepeatTopAndBottom,
        },
        judgement_line: c.judgement_line,
        keys_under_notes: c.keys_under_notes,
        upside_down: c.upside_down,
        barline_height: c.barline_height,
        colours: colours_dto(&c.colours),
    }
}

/// Absent or ≤ 0 means the narrowest column (`LegacySkin.cs` L152-156, research 06).
fn note_height_scale(c: &ManiaConfig) -> f32 {
    c.width_for_note_height_scale
        .filter(|w| *w > 0.0)
        .unwrap_or_else(|| c.column_width.iter().copied().fold(f32::INFINITY, f32::min))
}

fn rgba(c: Rgba) -> RgbaDto {
    [c.r, c.g, c.b, c.a]
}

/// Column backgrounds keep the raw alpha; the other colours get stable's zero-alpha rule here
/// so the renderer cannot apply the wrong one (`LegacyColourCompatibility.cs`, research 06).
fn colours_dto(c: &ManiaColours) -> ManiaColoursDto {
    let line = |c: Option<Rgba>| c.map(|c| rgba(c.disallow_zero_alpha()));
    ManiaColoursDto {
        column: c.column.iter().map(|c| c.map(rgba)).collect(),
        column_line: line(c.column_line),
        judgement_line: line(c.judgement_line),
        barline: line(c.barline),
        hold: line(c.hold),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::borrow::Cow;

    use wolluf_source_osu::skins::SkinsParams;

    use super::*;
    use crate::features::library::testkit::{Map, install};
    use crate::features::plays::testkit::Fixture;

    /// A leading `-`, spaces, `#` and a comma all occur in the pilot's folder names (research 06).
    const FOLDER: &str = "-  #Test, skin";
    const SKIN_INI: &str = "[General]\r\nName: Test Skin\r\nVersion: 2.5\r\n[Mania]\r\nKeys: 7\r\n\
        ColumnWidth: 40,42,42,42,42,42,42\r\nHitPosition: 428\r\nColour1: 10,20,30,0\r\n\
        ColourColumnLine: 1,2,3,0\r\nNoteImage0: notes\\custom\r\n[Mania]\r\nKeys: 4\r\n";
    const CFG: &str = "Username = Rosalind\r\nPassword = WOLLUF_SENTINEL_9f3a\r\n";

    /// Signature plus an IHDR chunk: enough for the header check, never a real skin image.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        out.extend_from_slice(&13u32.to_be_bytes());
        out.extend_from_slice(b"IHDR");
        out.extend_from_slice(&width.to_be_bytes());
        out.extend_from_slice(&height.to_be_bytes());
        out.extend_from_slice(&[8, 6, 0, 0, 0]);
        out.extend_from_slice(&[0; 4]);
        out
    }

    /// Synced so a catalog install exists; every skin file is written into the temp install only.
    async fn fixture(cfg_tail: &str) -> Fixture {
        let f = Fixture::new(&install(&[Map::k7("skins")], &[])).await;
        f.sync().await;
        f.write("osu!.fixture.cfg", format!("{CFG}{cfg_tail}").as_bytes());
        let skin = format!("Skins/{FOLDER}");
        f.write(format!("{skin}/skin.ini"), SKIN_INI.as_bytes());
        f.write(format!("{skin}/mania-key1.png"), &png(40, 100));
        f.write(format!("{skin}/mania-note1.png"), &png(64, 32));
        f.write(format!("{skin}/notes/custom@2x.png"), &png(80, 60));
        f.write("Skins/Bare/readme.txt", b"no skin.ini here");
        f
    }

    fn file_of<'d>(skin: &'d SkinDto, slot: &str) -> &'d SkinFileDto {
        let image = skin
            .images
            .iter()
            .find(|i| i.slot == slot)
            .unwrap_or_else(|| panic!("{slot} not resolved: {:?}", skin.images));
        &skin.files[usize::from(image.file)]
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn skin_list_lists_folders_with_cfg_defaults() {
        let f = fixture(&format!(
            "Skin = {FOLDER}\r\nPassword = WOLLUF_SENTINEL\r\nManiaSpeed = 30\r\nManiaSpeedBPMScale = 0\r\n"
        ))
        .await;

        let mut list = f.ctx.skins().list().await.unwrap();
        let mtimes: Vec<bool> = list.skins.iter().map(|s| s.ini_mtime.is_some()).collect();
        assert_eq!(mtimes, [true, false]);
        for skin in &mut list.skins {
            skin.ini_mtime = None;
        }
        assert!(
            !serde_json::to_string(&list)
                .unwrap()
                .contains("WOLLUF_SENTINEL"),
            "a cfg Password never reaches the webview"
        );
        assert_eq!(
            list,
            SkinListDto {
                skins: vec![
                    SkinEntryDto {
                        folder: FOLDER.into(),
                        name: Some("Test Skin".into()),
                        keymodes: vec![4, 7],
                        ini_mtime: None,
                    },
                    SkinEntryDto {
                        folder: "Bare".into(),
                        name: None,
                        keymodes: vec![],
                        ini_mtime: None,
                    },
                ],
                current: Some(FOLDER.into()),
                mania_speed: Some(30),
                mania_speed_bpm_scale: Some(false),
            }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn skin_list_current_is_none_unless_the_cfg_skin_is_listed() {
        for skin in ["Missing Skin", "-  #test, skin", "Bare/..", "Bare\\x"] {
            let f = fixture(&format!("Skin = {skin}\r\n")).await;
            let list = f.ctx.skins().list().await.unwrap();
            assert_eq!(list.current, None, "{skin:?}");
            assert_eq!(list.skins.len(), 2, "{skin:?}");
            assert_eq!(list.mania_speed, None, "{skin:?}");
        }

        let f = fixture("Skin = Bare\r\nManiaSpeed = 41\r\n").await;
        let list = f.ctx.skins().list().await.unwrap();
        assert_eq!(
            list.current.as_deref(),
            Some("Bare"),
            "a folder without skin.ini"
        );
        assert_eq!(list.mania_speed, None, "outside 1-40");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn skin_list_without_skins_or_catalog_is_empty() {
        let f = Fixture::new(&install(&[Map::k7("skins")], &[])).await;
        assert_eq!(
            f.ctx.skins().list().await.unwrap(),
            SkinListDto::default(),
            "no catalog install yet"
        );
        let err = f.ctx.skins().get(FOLDER, 7).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.message_key, keys::SKIN_UNAVAILABLE);

        f.sync().await;
        f.write(
            "osu!.fixture.cfg",
            format!("{CFG}ManiaSpeed = 24\r\n").as_bytes(),
        );
        let list = f.ctx.skins().list().await.unwrap();
        assert!(list.skins.is_empty(), "{list:?}");
        assert_eq!(list.current, None);
        assert_eq!(
            list.mania_speed,
            Some(24),
            "the cfg speed holds without Skins/"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn skin_get_returns_the_block_and_resolved_slots() {
        let f = fixture("").await;
        let skin = f.ctx.skins().get(FOLDER, 7).await.unwrap();

        assert_eq!(skin.folder, FOLDER);
        assert_eq!(skin.name.as_deref(), Some("Test Skin"));
        assert_eq!(skin.version, 2.5);
        assert_eq!(
            skin.config,
            ManiaConfigDto {
                keys: 7,
                column_width: vec![40.0, 42.0, 42.0, 42.0, 42.0, 42.0, 42.0],
                column_spacing: vec![0.0; 6],
                column_line_width: vec![2.0; 8],
                hit_position: 428.0,
                light_position: 413.0,
                width_for_note_height_scale: 40.0,
                note_body_style: NoteBodyStyleDto::RepeatBottom,
                judgement_line: true,
                keys_under_notes: false,
                upside_down: false,
                barline_height: 1.2,
                colours: ManiaColoursDto {
                    column: vec![Some([10, 20, 30, 0]), None, None, None, None, None, None],
                    column_line: Some([1, 2, 3, 255]),
                    judgement_line: None,
                    barline: None,
                    hold: None,
                },
            }
        );

        let key = file_of(&skin, "key.0");
        assert_eq!(
            key,
            &SkinFileDto {
                mime: "image/png".into(),
                scale: 1,
                width: 40,
                height: 100,
                base64: base64::encode(&png(40, 100)),
            }
        );
        let custom = file_of(&skin, "note.0");
        assert_eq!(
            (custom.scale, custom.width, custom.height),
            (2, 80, 60),
            "`notes\\custom` resolves to its @2x file"
        );
        assert_eq!(file_of(&skin, "note.2.tail"), file_of(&skin, "note.4"));
        let note1 = skin.images.iter().filter(|i| i.slot == "note.2").count();
        assert_eq!(note1, 1);
        assert_eq!(skin.files.len(), 3, "each file once: {:?}", skin.images);
        assert!(
            skin.images.contains(&SkinImageRefDto {
                slot: "key.6".into(),
                file: skin.images.iter().find(|i| i.slot == "key.0").unwrap().file,
            }),
            "{:?}",
            skin.images
        );
        assert!(
            skin.diagnostics.contains(&SkinDiagnosticDto {
                code: "skin.image_missing".into(),
                slot: Some("key.1".into()),
            }),
            "{:?}",
            skin.diagnostics
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn skin_get_falls_back_to_lazer_defaults() {
        let f = fixture("").await;
        let svc = f.ctx.skins();

        let four = svc.get(FOLDER, 4).await.unwrap();
        assert_eq!(four.config.column_width, vec![30.0; 4]);
        assert_eq!(four.config.hit_position, 402.0);
        assert_eq!(
            four.config.width_for_note_height_scale, 30.0,
            "the narrowest column"
        );

        let five = svc.get(FOLDER, 5).await.unwrap();
        assert_eq!(five.config.keys, 5);
        assert!(
            five.diagnostics.contains(&SkinDiagnosticDto {
                code: "skin.keys_block_missing".into(),
                slot: None,
            }),
            "{:?}",
            five.diagnostics
        );

        let bare = svc.get("Bare", 7).await.unwrap();
        assert_eq!(bare.name, None);
        assert_eq!(bare.version, 2.7, "no skin.ini loads as `latest`");
        assert_eq!(bare.config.note_body_style, NoteBodyStyleDto::RepeatBottom);
        assert!(bare.images.is_empty(), "{:?}", bare.images);
        assert!(bare.files.is_empty());
        assert!(
            bare.diagnostics
                .iter()
                .any(|d| d.code == "skin.ini_missing"),
            "{:?}",
            bare.diagnostics
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn skin_get_rejects_unknown_folders_keymodes_and_oversized_skins() {
        let f = fixture("").await;
        f.write("outside/skin.ini", SKIN_INI.as_bytes());
        let svc = f.ctx.skins();

        for folder in ["Nope", "../outside", "Bare/..", "", "-  #test, skin"] {
            let err = svc.get(folder, 7).await.unwrap_err();
            assert_eq!(err.code, ErrorCode::NotFound, "{folder:?}");
            assert_eq!(err.message_key, keys::SKIN_UNAVAILABLE, "{folder:?}");
        }
        for keymode in [0, 19] {
            let err = svc.get(FOLDER, keymode).await.unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidInput, "{keymode}");
            assert_eq!(
                err.args.get("keymode").map(String::as_str),
                Some(keymode.to_string().as_str())
            );
        }

        let capped = SkinParams {
            read: SkinsParams {
                max_skin_bytes: 40,
                ..SkinsParams::default()
            },
        };
        let err = SkinsService::new(&f.ctx)
            .with_params(capped)
            .get(FOLDER, 7)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::UnsupportedFormat);
        assert_eq!(err.message_key, keys::SKIN_TOO_LARGE);
        assert_eq!(err.args.get("maxBytes").map(String::as_str), Some("40"));
        let bytes: u64 = err.args.get("bytes").unwrap().parse().unwrap();
        assert!(bytes > 40, "{bytes}");
    }

    /// Leaves the graph as the macros built it: enough to prove no field needs a BigInt.
    struct Unrenamed;

    impl specta::Format for Unrenamed {
        fn map_types(
            &'_ self,
            types: &specta::Types,
        ) -> Result<Cow<'_, specta::Types>, specta::FormatError> {
            Ok(Cow::Owned(types.clone()))
        }

        fn map_type(
            &'_ self,
            _types: &specta::Types,
            dt: &specta::datatype::DataType,
        ) -> Result<Cow<'_, specta::datatype::DataType>, specta::FormatError> {
            Ok(Cow::Owned(dt.clone()))
        }
    }

    #[test]
    fn skin_dtos_need_no_bigint() {
        let types = specta::Types::default()
            .register::<SkinListDto>()
            .register::<SkinDto>();
        let ts = specta_typescript::Typescript::default()
            .export(&types, Unrenamed)
            .unwrap();
        assert!(!ts.contains("bigint"), "{ts}");
    }
}
