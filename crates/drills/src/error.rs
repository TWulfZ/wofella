use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DrillError {
    #[error("only osu!mania (mode 3) charts can be rate-copied")]
    UnsupportedMode,
    #[error("keysounded charts (storyboard samples or per-note sample files) are not rate-copied")]
    Keysounded,
    #[error("the chart names no audio file")]
    NoAudio,
    /// `line` is 1-based; 0 means a required section or line is absent.
    #[error("malformed chart at line {line}")]
    Malformed { line: usize },
    #[error("rate {rate_milli}/1000 is outside the allowed range")]
    RateOutOfRange { rate_milli: u16 },
    #[error("rate 1.00x would copy the chart unchanged")]
    IdentityRate,
    #[error("the chart is already a rate copy")]
    AlreadyRateCopy,
}
