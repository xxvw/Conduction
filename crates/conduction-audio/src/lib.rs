//! conduction-audio — リアルタイムオーディオエンジン。
//!
//! Phase 1 では rodio 経由の最小再生に留め、Phase 4 以降で cpal 直結 +
//! Conductor 層との lock-free チャネル構造に置き換える。
//!
//! アーキテクチャの全体像は要件定義 §3「システムアーキテクチャ」を参照。

#![forbid(unsafe_code)]

pub mod error;
pub mod player;

pub use error::{AudioError, AudioResult};
pub use player::Player;
