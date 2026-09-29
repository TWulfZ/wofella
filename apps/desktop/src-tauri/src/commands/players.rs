use wolluf_app::errors::IpcError;
use wolluf_app::features::players::dto::{
    self, AliasListDto, CreateProfileInput, DecideAliasInput, ProfileEntryDto,
    SetProfileAliasesInput,
};

use super::Ctx;
use crate::error::to_ipc;

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn players_list_aliases(ctx: Ctx<'_>) -> Result<AliasListDto, IpcError> {
    let list = ctx.players().list_aliases().await;
    list.and_then(AliasListDto::try_from).map_err(to_ipc)
}

/// Persisted profiles plus the virtual All players entry, each with its scopes for `keymode`.
#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn players_list_profiles(
    ctx: Ctx<'_>,
    keymode: u32,
) -> Result<Vec<ProfileEntryDto>, IpcError> {
    let keymode = dto::keymode(keymode).map_err(to_ipc)?;
    let entries = ctx.players().list_profiles(keymode).await;
    entries.and_then(dto::profile_entries).map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn players_set_profile_aliases(
    ctx: Ctx<'_>,
    input: SetProfileAliasesInput,
) -> Result<ProfileEntryDto, IpcError> {
    let (profile_id, alias_ids, merge_mode) = input.into_domain();
    let entry = ctx
        .players()
        .set_profile_aliases(profile_id, alias_ids, merge_mode)
        .await;
    entry.and_then(ProfileEntryDto::try_from).map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn players_decide_alias(
    ctx: Ctx<'_>,
    input: DecideAliasInput,
) -> Result<AliasListDto, IpcError> {
    let (batch, completes_wizard) = input.into_domain();
    let list = ctx.players().decide(batch, completes_wizard).await;
    list.and_then(AliasListDto::try_from).map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn players_create_profile(
    ctx: Ctx<'_>,
    input: CreateProfileInput,
) -> Result<ProfileEntryDto, IpcError> {
    let (label, alias_ids, merge_mode) = input.into_domain();
    let entry = ctx
        .players()
        .create_profile(label, alias_ids, merge_mode)
        .await;
    entry.and_then(ProfileEntryDto::try_from).map_err(to_ipc)
}

#[tauri::command]
#[specta::specta]
#[tracing::instrument(skip_all)]
pub async fn players_set_default(ctx: Ctx<'_>, profile_id: u32) -> Result<(), IpcError> {
    let profile_id = dto::profile_id(profile_id);
    ctx.players().set_default(profile_id).await.map_err(to_ipc)
}
