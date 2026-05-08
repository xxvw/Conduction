//! Tauri command handlers. UI から呼ばれるエンドポイント。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use conduction_analysis::{
    decode_to_pcm, estimate_beatgrid, generate_waveform, WaveformPreview, DEFAULT_WAVEFORM_BINS,
};
use conduction_core::Beat;
use serde::Serialize;
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
pub fn seek_deck(
    audio: State<'_, AudioHandle>,
    deck: String,
    position_sec: f64,
) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(
        &audio,
        AudioCommand::Seek {
            deck: id,
            position_sec,
        },
    )
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
    eprintln!("[cmd] import_track called: path={path}");
    info!(path = %path, "import_track invoked");
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
    let needs_waveform = library
        .with_library(|lib| lib.load_waveform(stored.id).map(|w| w.is_none()))
        .unwrap_or(true);
    info!(
        track_id = %stored.id,
        path = %stored.path.display(),
        needs_waveform,
        "import_track stored"
    );
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
    eprintln!("[cmd] analyze_track called: id={id}");
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
    eprintln!("[cmd] get_waveform called: id={id}");
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("invalid track id: {e}"))?;
    let track_id = TrackId::from_uuid(uuid);

    let result = library.with_library(|lib| {
        lib.load_waveform(track_id).map_err(|e| e.to_string())
    })?;

    // 波形が無く、まだ解析中でなければ、ここでバックグラウンド解析を開始する。
    // これにより、UI の useWaveform polling だけで自動解析が走る。
    if result.is_none() && library.claim_analyzing(track_id) {
        let path_opt = library
            .with_library(|lib| lib.get_track(track_id).map_err(|e| e.to_string()))?
            .map(|t| t.path);

        if let Some(path) = path_opt {
            let lib_shared = library.shared();
            let analyzing_set = library.analyzing_set();
            let analyze_path = path.clone();
            eprintln!("[cmd] get_waveform: spawning analyze thread for id={id}");
            let _ = std::thread::Builder::new()
                .name("analyze-on-demand".into())
                .spawn(move || {
                    info!(path = %analyze_path.display(), "on-demand analyze starting");
                    let started = std::time::Instant::now();
                    match analyze_and_save_internal(&lib_shared, track_id, &analyze_path) {
                        Ok(_) => info!(
                            path = %analyze_path.display(),
                            elapsed_ms = started.elapsed().as_millis() as u64,
                            "on-demand analyze completed"
                        ),
                        Err(e) => warn!(
                            error = %e,
                            path = %analyze_path.display(),
                            "on-demand analyze failed"
                        ),
                    }
                    analyzing_set.lock().remove(&track_id);
                });
        } else {
            // path 取得に失敗 → フラグを解放
            library.release_analyzing(track_id);
        }
    }

    eprintln!(
        "[cmd] get_waveform returning: id={id} hit={}",
        result.is_some()
    );
    Ok(result)
}

fn analyze_and_save_internal(
    library: &Arc<Mutex<Library>>,
    track_id: TrackId,
    path: &Path,
) -> Result<WaveformPreview, String> {
    let audio = decode_to_pcm(path).map_err(|e| e.to_string())?;
    let total_sec = audio.duration_sec();
    let wf = generate_waveform(&audio, DEFAULT_WAVEFORM_BINS);
    let estimate = estimate_beatgrid(&audio);

    let mut lib = library.lock();
    lib.save_waveform(track_id, &wf).map_err(|e| e.to_string())?;
    if let Some(est) = estimate {
        let beats = est.beats(total_sec);
        info!(
            bpm = est.bpm,
            confidence = est.confidence,
            beats = beats.len(),
            "beatgrid estimated"
        );
        lib.save_track_analysis(track_id, est.bpm, &beats)
            .map_err(|e| e.to_string())?;
    }
    Ok(wf)
}

/// UI に渡すビート1拍分の DTO。
#[derive(Debug, Clone, Serialize)]
pub struct BeatDto {
    pub position_sec: f64,
    pub is_downbeat: bool,
}

impl From<&Beat> for BeatDto {
    fn from(b: &Beat) -> Self {
        Self {
            position_sec: b.position_sec,
            is_downbeat: b.is_downbeat,
        }
    }
}

#[tauri::command]
pub fn get_track_beats(
    library: State<'_, LibraryHandle>,
    id: String,
) -> Result<Vec<BeatDto>, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("invalid track id: {e}"))?;
    library.with_library(|lib| {
        lib.load_beatgrid(TrackId::from_uuid(uuid))
            .map(|beats| beats.iter().map(BeatDto::from).collect())
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn list_tracks(library: State<'_, LibraryHandle>) -> Result<Vec<TrackSummary>, String> {
    eprintln!("[cmd] list_tracks called");
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
