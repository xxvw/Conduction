//! Tauri command handlers. UI から呼ばれるエンドポイント。

use std::path::PathBuf;

use conduction_core::TrackId;
use conduction_library::build_track_from_file;
use tauri::State;
use uuid::Uuid;

use crate::audio_engine::{parse_deck, parse_tempo_range, AudioCommand, AudioHandle, MixerSnapshot};
use crate::library_state::{LibraryHandle, TrackSummary};

type CmdResult<T = ()> = Result<T, String>;

fn send(handle: &AudioHandle, cmd: AudioCommand) -> CmdResult {
    handle.send(cmd).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn load_track(
    audio: State<'_, AudioHandle>,
    deck: String,
    path: String,
) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(
        &audio,
        AudioCommand::Load {
            deck: id,
            path: PathBuf::from(path),
        },
    )
}

#[tauri::command]
pub fn play(audio: State<'_, AudioHandle>, deck: String) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(&audio, AudioCommand::Play(id))
}

#[tauri::command]
pub fn pause(audio: State<'_, AudioHandle>, deck: String) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(&audio, AudioCommand::Pause(id))
}

#[tauri::command]
pub fn stop(audio: State<'_, AudioHandle>, deck: String) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(&audio, AudioCommand::Stop(id))
}

#[tauri::command]
pub fn set_crossfader(audio: State<'_, AudioHandle>, position: f32) -> CmdResult {
    send(&audio, AudioCommand::SetCrossfader(position))
}

#[tauri::command]
pub fn set_channel_volume(
    audio: State<'_, AudioHandle>,
    deck: String,
    volume: f32,
) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(
        &audio,
        AudioCommand::SetChannelVolume { deck: id, volume },
    )
}

#[tauri::command]
pub fn set_master_volume(audio: State<'_, AudioHandle>, volume: f32) -> CmdResult {
    send(&audio, AudioCommand::SetMasterVolume(volume))
}

#[tauri::command]
pub fn set_tempo_adjust(
    audio: State<'_, AudioHandle>,
    deck: String,
    adjust: f32,
) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(
        &audio,
        AudioCommand::SetTempoAdjust { deck: id, adjust },
    )
}

#[tauri::command]
pub fn set_tempo_range(
    audio: State<'_, AudioHandle>,
    deck: String,
    percent: u8,
) -> CmdResult {
    let id = parse_deck(&deck)?;
    let range = parse_tempo_range(percent)?;
    send(&audio, AudioCommand::SetTempoRange { deck: id, range })
}

#[tauri::command]
pub fn get_status(audio: State<'_, AudioHandle>) -> MixerSnapshot {
    audio.snapshot()
}

// ======== Library ========

#[tauri::command]
pub fn import_track(
    library: State<'_, LibraryHandle>,
    path: String,
) -> Result<TrackSummary, String> {
    let path_buf = PathBuf::from(&path);
    let track = build_track_from_file(&path_buf).map_err(|e| e.to_string())?;
    library.with_library(|lib| {
        let id = lib.upsert_track_by_path(&track).map_err(|e| e.to_string())?;
        let stored = lib
            .get_track(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "track disappeared after insert".to_string())?;
        Ok(TrackSummary::from_track(&stored))
    })
}

#[tauri::command]
pub fn list_tracks(library: State<'_, LibraryHandle>) -> Result<Vec<TrackSummary>, String> {
    library.with_library(|lib| {
        lib.list_tracks()
            .map(|tracks| tracks.iter().map(TrackSummary::from_track).collect())
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn delete_track(library: State<'_, LibraryHandle>, id: String) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("invalid track id: {e}"))?;
    library.with_library(|lib| {
        lib.delete_track(TrackId::from_uuid(uuid))
            .map_err(|e| e.to_string())
    })
}
