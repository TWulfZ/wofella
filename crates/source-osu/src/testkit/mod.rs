//! Encoders and builders for tests (feature `test-support`): this crate's tests, 003/004/006
//! tests and `cargo xtask fixtures`. Encoders are the exact inverse of the decoders.

mod osr;
mod osu_db;
mod scores;

pub use osr::{OsrBuilder, encode_osr};
#[cfg(test)]
pub(crate) use osu_db::write_beatmap;
pub use osu_db::{BeatmapBuilder, OsuDbBuilder, encode_osu_db};
pub use scores::{ScoreBuilder, ScoresDbBuilder, encode_scores_db};
