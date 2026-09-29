//! Pure decoders and encoders over `&[u8]` (spec 002). No IO here.

pub mod cfg;
pub mod collection_db;
pub mod osg;
pub mod osr;
pub mod osu_db;
pub mod reader;
pub mod replay_name;
pub mod score_header;
pub mod scores_db;
pub mod version;
pub mod writer;

pub use reader::{OsuString, Reader};
pub use version::FileKind;
pub use writer::Writer;
