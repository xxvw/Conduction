//! conduction-audio — リアルタイムオーディオエンジン。
//!
//! Phase 2a では rodio をバックエンドとした `OutputDevice` / `Deck` / `Mixer` を提供する。
//! Phase 4 以降で cpal 直結 + Conductor 層との lock-free チャネル構造に置き換える。
//!
//! アーキテクチャの全体像は要件定義 §3「システムアーキテクチャ」を参照。

#![forbid(unsafe_code)]

pub mod deck;
pub mod device;
pub mod error;
pub mod mixer;

pub use deck::{Deck, DeckId, CHANNEL_VOLUME_MAX, CHANNEL_VOLUME_MIN};
pub use device::OutputDevice;
pub use error::{AudioError, AudioResult};
pub use mixer::{
    CrossfaderCurve, Mixer, CROSSFADER_MAX, CROSSFADER_MIN, MASTER_VOLUME_MAX, MASTER_VOLUME_MIN,
};
