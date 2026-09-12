//! Unified CPAL output with a bounded worker renderer and consumed-frame transport clock.

#![forbid(unsafe_code)]

pub mod config;
pub mod deck;
pub mod device;
pub mod dsp;
pub mod error;
pub mod mixer;
pub mod pcm;
mod runtime;
mod voice;

pub use config::{AudioOutputConfig, AudioOutputStatus, MixingMode};
pub use deck::{Deck, DeckId, LoopState, TempoRange, CHANNEL_VOLUME_MAX, CHANNEL_VOLUME_MIN};
pub use device::{list_audio_outputs, AudioOutputDescriptor, OutputDevice};
pub use dsp::DspParams;
pub use error::{AudioError, AudioResult};
pub use mixer::{
    CrossfaderCurve, Mixer, CROSSFADER_MAX, CROSSFADER_MIN, MASTER_VOLUME_MAX, MASTER_VOLUME_MIN,
};
pub use pcm::PcmTrack;

#[cfg(test)]
mod offline_tests;
