//! Tauri-agnostic application layer (architecture §3). Shells call into it; it owns no UI.

pub mod clock;
pub mod errors;
pub mod features;
pub mod logging;
