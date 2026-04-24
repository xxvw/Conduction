//! Tauri command handlers. UI から呼ばれるエンドポイント。

use std::path::PathBuf;

use tauri::State;

use crate::audio_engine::{parse_deck, parse_tempo_range, AudioCommand, AudioHandle, MixerSnapshot};

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
