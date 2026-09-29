//! Read-only osu! stable adapter (D9): pure codecs over `&[u8]` plus IO helpers that never write
//! (specs 002, 003, 006).

pub mod codec;
pub mod diag;
pub mod error;
mod stable;

pub use diag::{DiagCode, Diagnostic, Diagnostics};
pub use error::{CodecError, SourceError};
