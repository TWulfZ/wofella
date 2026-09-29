//! Encoders and builders for tests (feature `test-support`): this crate's tests, 003/004/006
//! tests and `cargo xtask fixtures`. Encoders are the exact inverse of the decoders.

mod scores;

pub use scores::{ScoreBuilder, ScoresDbBuilder, encode_scores_db};
