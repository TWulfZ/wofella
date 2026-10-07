use wolluf_app::errors::IpcError;
use wolluf_app::features::labeling::dto::{
    AnchorDto, LabelEventDto, LabelStatsDto, LabelSubmitDto, LabelWindowDto, PatternDefDto,
    PatternExampleDto, SampleRequestDto, WindowOpDto,
};

use super::Ctx;
use crate::error::to_ipc;

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_taxonomy(ctx: Ctx<'_>, keymode: u8) -> Result<Vec<PatternDefDto>, IpcError> {
    ctx.labeling().taxonomy(keymode).map_err(to_ipc)
}

/// One synthetic preview per pattern of the keymode, in taxonomy order.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_pattern_examples(
    ctx: Ctx<'_>,
    keymode: u8,
) -> Result<Vec<PatternExampleDto>, IpcError> {
    ctx.labeling().pattern_examples(keymode).map_err(to_ipc)
}

/// `None` when no chart has a free window left.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_sample(
    ctx: Ctx<'_>,
    req: SampleRequestDto,
) -> Result<Option<LabelWindowDto>, IpcError> {
    ctx.labeling().sample(req).await.map_err(to_ipc)
}

/// Short keys or full ids to full pattern ids, in input order without repeats.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_resolve_patterns(
    ctx: Ctx<'_>,
    keymode: u8,
    tokens: Vec<String>,
) -> Result<Vec<String>, IpcError> {
    let ids = ctx.labeling().resolve_patterns(keymode, &tokens);
    ids.map(|ids| ids.iter().map(ToString::to_string).collect())
        .map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_reshape(
    ctx: Ctx<'_>,
    anchor: AnchorDto,
    op: WindowOpDto,
) -> Result<AnchorDto, IpcError> {
    ctx.labeling().reshape(anchor, op).await.map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_submit(ctx: Ctx<'_>, req: LabelSubmitDto) -> Result<LabelEventDto, IpcError> {
    ctx.labeling().submit(req).await.map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_undo(ctx: Ctx<'_>, event_id: String) -> Result<(), IpcError> {
    ctx.labeling().undo(&event_id).await.map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_stats(ctx: Ctx<'_>) -> Result<LabelStatsDto, IpcError> {
    ctx.labeling().stats().await.map_err(to_ipc)
}
