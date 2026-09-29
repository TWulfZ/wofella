//! Thin IPC commands (D11): map the DTO, call one app service, map the error.

pub mod app;
pub mod jobs;
pub mod setup;

use std::sync::Arc;

use wolluf_app::context::AppContext;

pub type Ctx<'a> = tauri::State<'a, Arc<AppContext>>;
