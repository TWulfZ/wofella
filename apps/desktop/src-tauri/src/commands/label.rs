use wolluf_app::errors::IpcError;
use wolluf_app::features::labeling::dto::{
    AnchorDto, ChartTimelineDto, ChartTimelineRequestDto, LabelEventDto, LabelStatsDto,
    LabelSubmitDto, LabelWindowDto, MoveWindowRequestDto, NowPlayingDto, NowPlayingRequestDto,
    PatternDefDto, PatternExampleDto, RandomRequestDto, SampleRequestDto, WindowAtRequestDto,
};

use super::Ctx;
use crate::error::to_ipc;

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_taxonomy(ctx: Ctx<'_>, keymode: u8) -> Result<Vec<PatternDefDto>, IpcError> {
    ctx.labeling().taxonomy(keymode).map_err(to_ipc)
}

/// One synthetic preview per pattern of the keymode, in taxonomy order. `layout_id` `None` draws
/// with the stored preference.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_pattern_examples(
    ctx: Ctx<'_>,
    keymode: u8,
    layout_id: Option<String>,
) -> Result<Vec<PatternExampleDto>, IpcError> {
    ctx.labeling()
        .pattern_examples(keymode, layout_id.as_deref())
        .await
        .map_err(to_ipc)
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

/// A free window inside the given chart; `NOT_FOUND` for a chart the catalog has not parsed,
/// `CONFLICT` when none of its windows is free.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_window_at(
    ctx: Ctx<'_>,
    req: WindowAtRequestDto,
) -> Result<LabelWindowDto, IpcError> {
    ctx.labeling().window_at(req).await.map_err(to_ipc)
}

/// Any eligible chart outside the stratified plan; `None` when no free window is left.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_random(
    ctx: Ctx<'_>,
    req: RandomRequestDto,
) -> Result<Option<LabelWindowDto>, IpcError> {
    ctx.labeling().random(req).await.map_err(to_ipc)
}

/// The chart osu! is playing, else the user's last replay; probes once per call.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_now_playing(
    ctx: Ctx<'_>,
    req: NowPlayingRequestDto,
) -> Result<Option<NowPlayingDto>, IpcError> {
    ctx.labeling().now_playing(req).await.map_err(to_ipc)
}

/// The window moved to start at `t0Ms`, same length, kept inside the chart.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_move_window(
    ctx: Ctx<'_>,
    req: MoveWindowRequestDto,
) -> Result<AnchorDto, IpcError> {
    ctx.labeling().move_window(req).await.map_err(to_ipc)
}

/// Note density over the whole chart and the user's labelled windows on it.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn label_chart_timeline(
    ctx: Ctx<'_>,
    req: ChartTimelineRequestDto,
) -> Result<ChartTimelineDto, IpcError> {
    ctx.labeling().chart_timeline(req).await.map_err(to_ipc)
}
