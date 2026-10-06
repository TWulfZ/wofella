//! Tauri-agnostic application layer (architecture §3). Shells call into it; it owns no UI.

mod base64;
pub mod clock;
pub mod context;
pub mod errors;
pub mod events;
pub mod features;
pub mod jobs;
pub mod logging;
pub mod watch;
