//! Thin IPC commands (D11): map the DTO, call one app service, map the error.

pub mod app;
pub mod chart;
pub mod jobs;
pub mod label;
pub mod players;
pub mod settings;
pub mod setup;
pub mod skin;

use std::sync::Arc;

use wolluf_app::context::AppContext;

pub type Ctx<'a> = tauri::State<'a, Arc<AppContext>>;
