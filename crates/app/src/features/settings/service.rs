//! `SettingsService`: the settings feature's only entry point for shells and other features
//! (D12).

use wolluf_core::Keymode;
use wolluf_engine::profile::{KeymodeProfile, Registry};
use wolluf_engine::window::layout_columns;
use wolluf_store::repo::ledger::settings;
use wolluf_store::{Conn, StoreError};

use super::dto::HandLayoutDto;
use crate::context::{AppContext, blocking_join_error};
use crate::errors::AppError;
use crate::features::library::dto::column_dto;

pub struct SettingsService<'a> {
    ctx: &'a AppContext,
}

impl<'a> SettingsService<'a> {
    pub fn new(ctx: &'a AppContext) -> Self {
        Self { ctx }
    }

    /// The column layouts a user may choose for `keymode`, the profile's default first.
    pub fn hand_layouts(&self, keymode: u8) -> Result<Vec<HandLayoutDto>, AppError> {
        Ok(profile(keymode)?
            .layouts()
            .iter()
            .map(|layout| HandLayoutDto {
                id: layout.id().to_owned(),
                columns: layout_columns(layout).into_iter().map(column_dto).collect(),
            })
            .collect())
    }

    /// The chosen layout id, else the profile's default. A stored id that no longer names a
    /// preset of the keymode falls back too, so a renamed preset never breaks the playfield.
    pub async fn hand_layout(&self, keymode: u8) -> Result<String, AppError> {
        let profile = profile(keymode)?;
        let user = self.ctx.user_db().clone();
        let stored =
            tokio::task::spawn_blocking(move || user.read(|c| settings::hand_layout(c, keymode)))
                .await
                .map_err(blocking_join_error)??;
        Ok(stored
            .and_then(|id| profile.layout_by_id(&id))
            .unwrap_or_else(|| profile.layout())
            .id()
            .to_owned())
    }

    /// `INVALID_INPUT` unless `layout_id` is a preset of the keymode.
    pub async fn set_hand_layout(&self, keymode: u8, layout_id: &str) -> Result<(), AppError> {
        let layout = profile(keymode)?
            .layout_by_id(layout_id)
            .ok_or_else(|| AppError::invalid_input().with_arg("layoutId", layout_id))?;
        let id = layout.id().to_owned();
        let user = self.ctx.user_db().clone();
        tokio::task::spawn_blocking(move || {
            user.write(move |tx| settings::set_hand_layout(tx, keymode, &id))
        })
        .await
        .map_err(blocking_join_error)??;
        Ok(())
    }

    /// Whether a finished self map flashes the window (ADR 0020); off by default.
    pub async fn session_notify(&self) -> Result<bool, AppError> {
        let user = self.ctx.user_db().clone();
        let on = tokio::task::spawn_blocking(move || user.read(session_notify))
            .await
            .map_err(blocking_join_error)??;
        Ok(on)
    }

    pub async fn set_session_notify(&self, on: bool) -> Result<(), AppError> {
        let user = self.ctx.user_db().clone();
        tokio::task::spawn_blocking(move || {
            user.write(move |tx| settings::set_session_notify(tx, on))
        })
        .await
        .map_err(blocking_join_error)??;
        Ok(())
    }
}

/// [`SettingsService::session_notify`] inside a caller's read, for code that must not hold the
/// context (the session tracker).
pub(crate) fn session_notify(c: Conn<'_>) -> Result<bool, StoreError> {
    settings::session_notify(c)
}

fn profile(keymode: u8) -> Result<&'static KeymodeProfile, AppError> {
    Keymode::new(keymode)
        .ok()
        .and_then(|k| Registry::builtin().profile(k))
        .ok_or_else(|| AppError::invalid_input().with_arg("keymode", keymode.to_string()))
}

#[cfg(test)]
mod tests {
    use wolluf_core::{ErrorCode, FixedClock, UnixUs};

    use super::*;
    use crate::context::AppPaths;
    use crate::features::library::dto::{ColumnDto, FingerDto, HandDto};

    const T0: UnixUs = UnixUs(1_790_637_236_636_000);

    fn context(dir: &std::path::Path) -> AppContext {
        AppContext::open(
            AppPaths::from_data_dir(dir.join("data")),
            std::sync::Arc::new(FixedClock::new(T0)),
        )
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn hand_layouts_list_the_keymode_presets_default_first() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let layouts = ctx.settings().hand_layouts(7).unwrap();
        let ids: Vec<&str> = layouts.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "k7.313_right_thumb",
                "k7.313_left_thumb",
                "k7.43",
                "k7.34",
                "k7.both_thumbs"
            ]
        );
        assert_eq!(layouts[1].columns.len(), 7);
        assert_eq!(
            layouts[1].columns[3],
            ColumnDto {
                hand: HandDto::Left,
                finger: FingerDto::Thumb
            }
        );
        let err = ctx.settings().hand_layouts(5).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn session_notify_is_off_until_turned_on_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        assert!(!ctx.settings().session_notify().await.unwrap());
        ctx.settings().set_session_notify(true).await.unwrap();
        assert!(ctx.settings().session_notify().await.unwrap());
        drop(ctx);
        let reopened = context(dir.path());
        assert!(reopened.settings().session_notify().await.unwrap());
        reopened.settings().set_session_notify(false).await.unwrap();
        assert!(!reopened.settings().session_notify().await.unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn hand_layout_defaults_to_the_profile_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let settings = ctx.settings();
        assert_eq!(settings.hand_layout(7).await.unwrap(), "k7.313_right_thumb");
        settings
            .set_hand_layout(7, "k7.313_left_thumb")
            .await
            .unwrap();
        assert_eq!(settings.hand_layout(7).await.unwrap(), "k7.313_left_thumb");

        for bad in ["k4.generic", "nope", ""] {
            let err = settings.set_hand_layout(7, bad).await.unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidInput, "{bad:?}");
        }
        assert_eq!(
            settings
                .set_hand_layout(5, "k5.generic")
                .await
                .unwrap_err()
                .code,
            ErrorCode::InvalidInput
        );
        assert_eq!(settings.hand_layout(7).await.unwrap(), "k7.313_left_thumb");

        drop(ctx);
        let reopened = context(dir.path());
        assert_eq!(
            reopened.settings().hand_layout(7).await.unwrap(),
            "k7.313_left_thumb"
        );
    }
}
