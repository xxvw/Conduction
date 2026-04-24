use std::path::PathBuf;

use thiserror::Error;

pub type AudioResult<T> = Result<T, AudioError>;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("no default audio output device available")]
    NoDefaultDevice,

    #[error("failed to open output stream: {0}")]
    Stream(String),

    #[error("failed to open file {path}: {source}")]
    FileOpen {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to decode audio: {0}")]
    Decode(String),

    #[error("playback error: {0}")]
    Playback(String),
}
