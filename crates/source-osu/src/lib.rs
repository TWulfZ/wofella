//! Read-only osu! stable adapter (D9): pure codecs over `&[u8]` plus IO helpers that never write
//! (specs 002, 003, 006).

pub mod cfg_files;
pub mod codec;
pub mod diag;
pub mod error;
pub mod install;
pub mod paths;
pub mod probe;
mod process;
pub mod replay_dir;
pub mod skins;
pub mod snapshot;
pub mod song_image;
pub mod songs;
mod stable;
#[cfg(any(test, feature = "test-support"))]
pub mod testkit;
pub mod watch;

pub use diag::{DiagCode, Diagnostic, Diagnostics};
pub use error::{CodecError, SourceError};
