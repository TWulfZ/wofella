use wolluf_app::errors::IpcError;
use wolluf_app::features::players::dto::{self, EntryRefDto, MergeModeDto};
use wolluf_app::features::players::scope::MergeMode;
use wolluf_app::features::preview::dto::SkillPreviewDto;

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
