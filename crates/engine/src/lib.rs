//! Registry, keymode profiles and the versioned derivation stages (architecture §3, §5.5).

pub mod error;
pub mod examples;
pub mod labels;
pub mod preview;
pub mod profile;
pub mod render;
pub mod rows_blob;
pub mod stage;
pub mod taxonomy;
pub mod window;

pub use error::EngineError;
/// app may not depend on chart (D1), so the pure `[Events]` read reaches it through here.
pub use wolluf_chart::decode::events::background_name;
