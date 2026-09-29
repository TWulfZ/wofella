//! Encoders and builders for tests (feature `test-support`): this crate's tests, 003/004/006
//! tests and `cargo xtask fixtures`. Encoders are the exact inverse of the decoders.

mod install;
mod osg;
mod osr;
mod osu_db;
mod probe;
mod scores;

pub use install::FakeInstall;
pub use osg::encode_osg;
pub use osr::{OsrBuilder, encode_osr};
#[cfg(test)]
pub(crate) use osu_db::write_beatmap;
pub use osu_db::{BeatmapBuilder, OsuDbBuilder, encode_osu_db};
pub use probe::FakeProbe;
pub use scores::{ScoreBuilder, ScoresDbBuilder, encode_scores_db};
