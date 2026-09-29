//! Shared domain vocabulary: ids, time, keymode, version keys, error codes, clock (architecture §5.1). No algorithms, no IO.

pub mod digest;
pub mod error;
pub mod id;
pub mod keymode;
pub mod time;

pub use digest::{AliasId, BlobSha256, ChartMd5, Game, PlayId, ProfileId};
pub use error::CoreError;
pub use id::{AxisId, PatternId, StageId};
pub use keymode::{ColMask, Keymode};
pub use time::{DotNetTicks, FileTime, RateMilli, TimeUs, UnixUs};
