//! Fatal chart decode errors. Oddities that still yield a chart are `Diagnostics` instead.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChartError {
    #[error("unsupported game mode {0}: only osu!mania (mode 3) charts are decoded")]
    UnsupportedMode(i32),
    #[error("game mode value {0:?} is not an integer")]
    InvalidMode(String),
    #[error("chart bytes could not be read: {0}")]
    Read(#[from] std::io::Error),
}
