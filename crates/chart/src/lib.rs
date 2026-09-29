//! Normalized chart model, decoders and column layouts (architecture §3, chart crate). Pure over
//! bytes: no IO, no clock, deterministic outputs.

pub mod decode;
pub mod diag;
pub mod error;
pub mod layout;
pub mod model;
#[cfg(any(test, feature = "test-support"))]
pub mod testkit;

pub use decode::{ChartDecoder, Decoded, OsuDecoder};
pub use diag::{DiagCode, Diagnostic, Diagnostics};
pub use error::ChartError;
pub use layout::{Finger, Hand, Layout};
pub use model::{Chart, ChartMeta, LnPair, Note, NoteKind, Row, TimingKind, TimingPoint};
