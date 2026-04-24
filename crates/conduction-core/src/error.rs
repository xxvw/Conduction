use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid key notation: {0}")]
    InvalidKey(String),

    #[error("invalid cue: {0}")]
    InvalidCue(String),

    #[error("invalid range: start {start} >= end {end}")]
    InvalidRange { start: f64, end: f64 },

    #[error("value out of range: {field} = {value} (expected {expected})")]
    OutOfRange {
        field: &'static str,
        value: f64,
        expected: &'static str,
    },
}
