//! conduction-analysis — 楽曲解析。
//!
//! Phase 3b-1: PCM デコード + 3 バンド波形プレビュー生成。
//! Phase 3b-5 以降で BPM / キー / 構造解析を追加する。

#![forbid(unsafe_code)]

pub mod decode;
pub mod error;
pub mod waveform;

pub use decode::{decode_to_pcm, DecodedAudio};
pub use error::{AnalysisError, AnalysisResult};
pub use waveform::{generate_waveform, WaveformPreview, DEFAULT_WAVEFORM_BINS};
