//! Native, experimental Pro DJ Link networking and read-only library serving.
//!
//! Packet formats are independently implemented from the public DJ Link
//! Ecosystem Analysis. Hardware interoperability is deliberately not asserted.

mod dbserver;
mod nfs;
mod service;
mod wire;

pub use dbserver::encode_three_band_waveform;
use serde::{Deserialize, Serialize};
pub use service::LinkHandle;
use std::{net::Ipv4Addr, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkConfig {
    pub enabled: bool,
    pub interface_ip: Ipv4Addr,
    pub broadcast_ip: Ipv4Addr,
    pub mac_address: [u8; 6],
    pub source_deck: String,
    pub latency_ms: f64,
    pub library_enabled: bool,
    pub preferred_player: Option<u8>,
}

impl Default for LinkConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interface_ip: Ipv4Addr::UNSPECIFIED,
            broadcast_ip: Ipv4Addr::BROADCAST,
            mac_address: [0; 6],
            source_deck: "A".into(),
            latency_ms: 0.0,
            library_enabled: false,
            preferred_player: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LibrarySnapshot {
    pub tracks: Vec<LinkTrack>,
    pub playlists: Vec<LinkPlaylist>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LinkTrack {
    pub id: u32,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub duration_ms: u32,
    pub bpm: f64,
    pub path: PathBuf,
    pub byte_size: u64,
    pub beats: Vec<LinkBeat>,
    pub cues: Vec<LinkCue>,
    pub waveform_preview: Vec<u8>,
    pub waveform_detail: Vec<u8>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LinkBeat {
    pub time_ms: u32,
    pub beat: u8,
    pub bpm: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LinkCue {
    pub slot: u8,
    pub time_ms: u32,
    pub end_ms: Option<u32>,
    pub name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LinkPlaylist {
    pub id: u32,
    pub name: String,
    pub track_ids: Vec<u32>,
}

/// Clock samples must come from the audio engine, not a GUI wall clock.
/// `beat` is the one-based absolute beat; `beat_phase` is in [0, 1).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalClock {
    pub deck: String,
    pub playing: bool,
    pub bpm: f64,
    pub beat: u32,
    pub beat_phase: f64,
    pub track_id: Option<u32>,
    pub synced: bool,
    pub grid_available: bool,
}

impl Default for LocalClock {
    fn default() -> Self {
        Self {
            deck: "A".into(),
            playing: false,
            bpm: 0.0,
            beat: 0,
            beat_phase: 0.0,
            track_id: None,
            synced: false,
            grid_available: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkDevice {
    pub device_number: u8,
    pub name: String,
    pub ip: Ipv4Addr,
    pub kind: u8,
    pub bpm: Option<f64>,
    pub playing: bool,
    pub synced: bool,
    pub master: bool,
    pub beat: u32,
    pub track_id: Option<u32>,
    pub last_seen_micros: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkStatus {
    pub running: bool,
    pub hardware_verified: bool,
    pub capability: String,
    pub player_number: Option<u8>,
    pub source_number: u8,
    pub master_number: Option<u8>,
    pub devices: Vec<LinkDevice>,
    pub library_tracks: usize,
    pub last_error: Option<String>,
}

impl Default for LinkStatus {
    fn default() -> Self {
        Self {
            running: false,
            hardware_verified: false,
            capability: "experimental".into(),
            player_number: None,
            source_number: 17,
            master_number: None,
            devices: Vec::new(),
            library_tracks: 0,
            last_error: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinkEvent {
    Beat {
        device_number: u8,
        bpm: f64,
        beat: u8,
        received_at_micros: u64,
    },
    Status {
        device: LinkDevice,
    },
    SyncCommand {
        enabled: bool,
    },
    MasterChanged {
        device_number: Option<u8>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("Invalid Pro DJ Link configuration: {0}")]
    InvalidConfig(String),
    #[error("Pro DJ Link network error: {0}")]
    Io(#[from] std::io::Error),
}
