use std::path::PathBuf;

use thiserror::Error;

pub type LibraryResult<T> = Result<T, LibraryError>;

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("core error: {0}")]
    Core(#[from] conduction_core::CoreError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("schema error: {0}")]
    Schema(String),

    #[error("unsupported data state: {0}")]
    Unsupported(String),

    #[error("track not found for path: {0}")]
    TrackNotFound(PathBuf),

    #[error("metadata extraction failed for {path}: {source}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: lofty::error::LoftyError,
    },
}
