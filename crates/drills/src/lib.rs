//! Rendered drills: rate copies first (ADR 0025).

mod decimal;
mod error;
mod rate_copy;

pub use error::DrillError;
pub use rate_copy::{RateCopy, RateCopyParams, rate_copy};
