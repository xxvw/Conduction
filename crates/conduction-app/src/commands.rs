//! Tauri command handlers. UI から呼ばれるエンドポイント。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use conduction_analysis::{decode_to_pcm, generate_waveform, WaveformPreview, DEFAULT_WAVEFORM_BINS};
use conduction_core::TrackId;
use conduction_library::{build_track_from_file, Library};
use parking_lot::Mutex;
use tauri::State;
use tracing::{info, warn};
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
    let stored = library.with_library(|lib| -> Result<_, String> {
        let id = lib.upsert_track_by_path(&track).map_err(|e| e.to_string())?;
        let stored = lib
            .get_track(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "track disappeared after insert".to_string())?;
        Ok(stored)
    })?;

    // 既存の波形が無ければバックグラウンドで生成。UI には即座に return。
    // tauri::async_runtime::spawn_blocking は環境によって即時に走らないことがあるため、
    // 確実な std::thread::spawn を使う。
    let needs_waveform = library
        .with_library(|lib| lib.load_waveform(stored.id).map(|w| w.is_none()))
        .unwrap_or(true);
    if needs_waveform {
        let lib_shared = library.shared();
        let track_id = stored.id;
        let analyze_path = stored.path.clone();
        let _ = std::thread::Builder::new()
            .name("analyze-import".into())
            .spawn(move || {
                info!(path = %analyze_path.display(), "background analyze starting");
                let started = std::time::Instant::now();
                match analyze_and_save_internal(&lib_shared, track_id, &analyze_path) {
                    Ok(_) => info!(
                        path = %analyze_path.display(),
                        elapsed_ms = started.elapsed().as_millis() as u64,
                        "background analyze completed"
                    ),
                    Err(e) => warn!(
                        error = %e,
                        path = %analyze_path.display(),
                        "background analyze failed"
                    ),
                }
            });
    }

    Ok(TrackSummary::from_track(&stored))
}

#[tauri::command]
pub fn analyze_track(
    library: State<'_, LibraryHandle>,
    id: String,
) -> Result<WaveformPreview, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("invalid track id: {e}"))?;
    let track_id = TrackId::from_uuid(uuid);

    let path = library.with_library(|lib| -> Result<PathBuf, String> {
        let t = lib
            .get_track(track_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("track not found: {id}"))?;
        Ok(t.path)
    })?;

    info!(path = %path.display(), "analyze_track starting");
    let started = std::time::Instant::now();
    let result = analyze_and_save_internal(&library.shared(), track_id, &path);
    info!(elapsed_ms = started.elapsed().as_millis() as u64, ok = result.is_ok(), "analyze_track finished");
    result
}

#[tauri::command]
pub fn get_waveform(
    library: State<'_, LibraryHandle>,
    id: String,
) -> Result<Option<WaveformPreview>, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("invalid track id: {e}"))?;
    library.with_library(|lib| {
        lib.load_waveform(TrackId::from_uuid(uuid))
            .map_err(|e| e.to_string())
    })
}

fn analyze_and_save_internal(
    library: &Arc<Mutex<Library>>,
    track_id: TrackId,
    path: &Path,
) -> Result<WaveformPreview, String> {
    let audio = decode_to_pcm(path).map_err(|e| e.to_string())?;
    let wf = generate_waveform(&audio, DEFAULT_WAVEFORM_BINS);
    let lib = library.lock();
    lib.save_waveform(track_id, &wf).map_err(|e| e.to_string())?;
    Ok(wf)
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
