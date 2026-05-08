use std::path::PathBuf;

use thiserror::Error;

pub type AnalysisResult<T> = Result<T, AnalysisError>;

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("io error opening {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("decode error: {0}")]
    Decode(String),

    #[error("unsupported audio format: {0}")]
    Unsupported(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),
}
