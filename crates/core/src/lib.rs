//! Shared domain vocabulary: ids, time, keymode, version keys, error codes, clock (architecture §5.1). No algorithms, no IO.

pub mod error;
pub mod id;

pub use error::CoreError;
pub use id::{AxisId, PatternId, StageId};
