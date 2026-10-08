use wolluf_app::errors::IpcError;
use wolluf_app::features::players::dto::{self, EntryRefDto, MergeModeDto};
use wolluf_app::features::players::scope::MergeMode;
use wolluf_app::features::preview::dto::{RecsModeDto, RecsPreviewDto, SkillPreviewDto};

use super::Ctx;
use crate::error::to_ipc;

/// One uncalibrated preview per resolved scope of `entry` (ADR 0024); `merge` overrides the
/// profile's own merge mode for this read only.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn preview_skill(
    ctx: Ctx<'_>,
    entry: EntryRefDto,
    keymode: u8,
    merge: Option<MergeModeDto>,
) -> Result<Vec<SkillPreviewDto>, IpcError> {
    let keymode = dto::keymode(u32::from(keymode)).map_err(to_ipc)?;
    let merge = merge.map(MergeMode::from);
    let previews = ctx.preview().skill(entry.into(), keymode, merge).await;
    previews.map_err(to_ipc)
}

/// Charts × rate around the first resolved scope's preview rating (ADR 0024); `skillset` is a
/// MinaCalc id, read in `skillset` mode only.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn preview_recs(
    ctx: Ctx<'_>,
    entry: EntryRefDto,
    keymode: u8,
    mode: RecsModeDto,
    skillset: Option<String>,
    merge: Option<MergeModeDto>,
) -> Result<RecsPreviewDto, IpcError> {
    let keymode = dto::keymode(u32::from(keymode)).map_err(to_ipc)?;
    let merge = merge.map(MergeMode::from);
    let preview = ctx.preview();
    let recs = preview.recs(entry.into(), keymode, mode, skillset, merge, None);
    recs.await.map_err(to_ipc)
}
