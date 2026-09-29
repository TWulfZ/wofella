use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error("invalid stable id {0:?}: expected 1-64 bytes of [a-z0-9_] segments joined by '.'")]
    InvalidStableId(String),
    #[error("invalid FILETIME decimal {0:?}")]
    InvalidFileTime(String),
    #[error("rate must be greater than zero")]
    ZeroRate,
    #[error("keymode must have 1-16 columns, got {0}")]
    InvalidKeymode(u8),
    #[error("column {col} is outside a {keymode}-column keymode")]
    ColumnOutOfRange { col: u8, keymode: u8 },
    #[error("invalid {type_name} hex {input:?}: expected lowercase hex of the exact length")]
    InvalidHex {
        type_name: &'static str,
        input: String,
    },
    #[error("unknown game {0:?}")]
    UnknownGame(String),
}
