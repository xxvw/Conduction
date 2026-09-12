use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All absolute controller values are normalized to 0..=1. Deck names are A/B.
/// MIDI and UI operations must be applied through the same application dispatcher.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ControllerAction {
    PlayPause {
        deck: String,
    },
    Cue {
        deck: String,
        pressed: bool,
    },
    JogTouch {
        deck: String,
        touched: bool,
    },
    Jog {
        deck: String,
        delta: f32,
    },
    Nudge {
        deck: String,
        amount: f32,
    },
    Tempo {
        deck: String,
        value: f32,
    },
    Sync {
        deck: String,
    },
    Eq {
        deck: String,
        band: String,
        value: f32,
    },
    Filter {
        deck: String,
        value: f32,
    },
    Fader {
        deck: String,
        value: f32,
    },
    Master {
        value: f32,
    },
    Crossfader {
        value: f32,
    },
    HeadphoneVolume {
        value: f32,
    },
    HeadphoneMix {
        value: f32,
    },
    HeadphoneCue {
        deck: String,
    },
    HotCue {
        deck: String,
        slot: u8,
        operation: String,
    },
    Loop {
        deck: String,
        operation: String,
    },
    Browse {
        delta: i32,
    },
    LoadSelected {
        deck: String,
    },
    Fx {
        deck: String,
        parameter: String,
        value: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "control", rename_all = "snake_case")]
pub enum Control {
    PlayPause {
        deck: String,
    },
    Cue {
        deck: String,
    },
    JogTouch {
        deck: String,
    },
    Jog {
        deck: String,
    },
    Nudge {
        deck: String,
    },
    Tempo {
        deck: String,
    },
    Sync {
        deck: String,
    },
    Eq {
        deck: String,
        band: String,
    },
    Filter {
        deck: String,
    },
    Fader {
        deck: String,
    },
    Master,
    Crossfader,
    HeadphoneVolume,
    HeadphoneMix,
    HeadphoneCue {
        deck: String,
    },
    HotCue {
        deck: String,
        slot: u8,
        operation: String,
    },
    Loop {
        deck: String,
        operation: String,
    },
    Browse,
    LoadSelected {
        deck: String,
    },
    Fx {
        deck: String,
        parameter: String,
    },
    /// Local selector state for hardware sharing one FX strip among channels.
    /// Does not dispatch an audio operation. Fx with deck "selected" fans out.
    FxTarget {
        deck: String,
        exclusive: bool,
    },
}

/// Channels and note/controller numbers are zero based; 0xB0 is channel 0 CC.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MidiMessage {
    Note { channel: u8, number: u8 },
    ControlChange { channel: u8, number: u8 },
    PitchBend { channel: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelativeEncoding {
    /// 0x40 is stationary; 0x41 is +1; 0x3f is -1 (Pioneer jogs).
    Offset64,
    /// 0x01 is +1, 0x7f is -1 (Pioneer browse encoders).
    TwosComplement,
    /// Values 1..63 are positive; 65..127 are negative magnitudes.
    SignMagnitude,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum InputMode {
    Button,
    /// Positive event pulses with no guaranteed corresponding NoteOff.
    Trigger,
    Gate,
    Absolute7,
    /// Binding is the MSB CC; lsb is the second CC number on the same channel.
    Absolute14 {
        lsb: u8,
    },
    PitchBend14,
    Relative {
        encoding: RelativeEncoding,
        step: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub message: MidiMessage,
    pub control: Control,
    pub mode: InputMode,
    #[serde(default)]
    pub pickup: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "control", rename_all = "snake_case")]
pub enum LedControl {
    Playing { deck: String },
    Cue { deck: String },
    Sync { deck: String },
    HeadphoneCue { deck: String },
    HotCue { deck: String, slot: u8 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedBinding {
    pub message: MidiMessage,
    pub control: LedControl,
    pub on_value: u8,
    pub off_value: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiProfile {
    pub id: String,
    pub name: String,
    pub experimental: bool,
    pub sources: Vec<String>,
    pub warnings: Vec<String>,
    pub bindings: Vec<Binding>,
    pub leds: Vec<LedBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiConfig {
    pub input_port: String,
    pub output_port: Option<String>,
    pub profile: String,
    /// CDJ profiles default to deck A; select A or B for each physical player.
    pub deck: Option<String>,
    /// Empty uses the documented profile. Nonempty replaces its input map.
    #[serde(default)]
    pub bindings: Vec<Binding>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MidiDeckState {
    pub playing: bool,
    pub cue: bool,
    pub sync: bool,
    pub headphones_cue: bool,
    pub hot_cues: [bool; 8],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControllerState {
    pub decks: HashMap<String, MidiDeckState>,
    /// Confirmed normalized values keyed by Control::key(), e.g. "tempo:A",
    /// "eq:A:low", "fader:A", "master", "crossfader", "headphone_mix".
    /// Values enable pickup; absent values suppress pickup-bound controls.
    pub values: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiPortInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MidiDevices {
    pub inputs: Vec<MidiPortInfo>,
    pub outputs: Vec<MidiPortInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiConnectionStatus {
    pub config: MidiConfig,
    pub profile_name: String,
    pub connected: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MidiStatus {
    pub connections: Vec<MidiConnectionStatus>,
    pub dropped_messages: u64,
    pub error: Option<String>,
}
