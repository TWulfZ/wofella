use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error("invalid stable id {0:?}: expected 1-64 bytes of [a-z0-9_] segments joined by '.'")]
    InvalidStableId(String),
}
