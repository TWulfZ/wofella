//! Chart decoders (D8 extension axis: osu! now, other games later, architecture §9.5).

pub mod events;
pub mod osu;

pub use osu::OsuDecoder;

use crate::diag::Diagnostics;
use crate::error::ChartError;
use crate::model::Chart;

pub trait ChartDecoder {
    /// Stable id of the source format.
    fn format_id(&self) -> &'static str;

    fn decode(&self, bytes: &[u8]) -> Result<Decoded, ChartError>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub chart: Chart,
    pub diagnostics: Diagnostics,
}
