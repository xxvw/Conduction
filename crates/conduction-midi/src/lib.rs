//! Native MIDI transport and documented two-deck controller profiles.
//! All hardware profiles remain experimental until a physical-device soak test.
mod decoder;
mod profiles;
mod service;
mod types;

pub use decoder::{led_messages, validate_bindings, MidiDecoder};
pub use profiles::builtin_profiles;
pub use service::{MidiError, MidiService};
pub use types::*;

#[cfg(test)]
mod tests;
