//! Tauri command handlers. UI から呼ばれるエンドポイント。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use conduction_analysis::{
    decode_to_pcm, estimate_beatgrid, estimate_key, generate_waveform, WaveformPreview,
    DEFAULT_WAVEFORM_BINS,
};
use conduction_audio::OutputDevice;
use conduction_core::{Beat, Cue, CueId, CueType, MixRole};
use serde::{Deserialize, Serialize};
use conduction_core::TrackId;
use conduction_library::{build_track_from_file, Library};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, State};
use tracing::{info, warn};
use uuid::Uuid;

use crate::audio_engine::{parse_deck, parse_tempo_range, AudioCommand, AudioHandle, MixerSnapshot};
use conduction_conductor::Template;
use crate::library_state::{LibraryHandle, TrackSummary};
use crate::settings::{AppSettings, SettingsHandle};
use crate::system_stats::{ResourceStats, SystemStatsHandle};

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
pub fn loop_in(audio: State<'_, AudioHandle>, deck: String, position_sec: f64) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(&audio, AudioCommand::LoopIn { deck: id, position_sec })
}

#[tauri::command]
pub fn loop_out(audio: State<'_, AudioHandle>, deck: String, position_sec: f64) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(&audio, AudioCommand::LoopOut { deck: id, position_sec })
}

#[tauri::command]
pub fn loop_toggle(audio: State<'_, AudioHandle>, deck: String) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(&audio, AudioCommand::LoopToggle(id))
}

#[tauri::command]
pub fn loop_clear(audio: State<'_, AudioHandle>, deck: String) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(&audio, AudioCommand::LoopClear(id))
}

#[tauri::command]
pub fn set_eq(
    audio: State<'_, AudioHandle>,
    deck: String,
    band: String,
    db: f32,
) -> CmdResult {
    let id = parse_deck(&deck)?;
    let cmd = match band.as_str() {
        "low" | "Low" => AudioCommand::SetEqLow { deck: id, db },
        "mid" | "Mid" => AudioCommand::SetEqMid { deck: id, db },
        "high" | "High" => AudioCommand::SetEqHigh { deck: id, db },
        other => return Err(format!("invalid eq band: {other}")),
    };
    send(&audio, cmd)
}

#[tauri::command]
pub fn set_filter(audio: State<'_, AudioHandle>, deck: String, value: f32) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(&audio, AudioCommand::SetFilter { deck: id, value })
}

#[tauri::command]
pub fn set_echo(
    audio: State<'_, AudioHandle>,
    deck: String,
    wet: f32,
    time_ms: f32,
    feedback: f32,
) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(
        &audio,
        AudioCommand::SetEcho {
            deck: id,
            wet,
            time_ms,
            feedback,
        },
    )
}

#[tauri::command]
pub fn set_reverb(
    audio: State<'_, AudioHandle>,
    deck: String,
    wet: f32,
    room: f32,
) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(
        &audio,
        AudioCommand::SetReverb { deck: id, wet, room },
    )
}

#[tauri::command]
pub fn set_cue_send(audio: State<'_, AudioHandle>, deck: String, value: f32) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(&audio, AudioCommand::SetCueSend { deck: id, value })
}

#[tauri::command]
pub fn set_key_lock(audio: State<'_, AudioHandle>, deck: String, on: bool) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(&audio, AudioCommand::SetKeyLock { deck: id, on })
}

#[tauri::command]
pub fn set_pitch_offset(
    audio: State<'_, AudioHandle>,
    deck: String,
    semitones: f32,
) -> CmdResult {
    let id = parse_deck(&deck)?;
    send(
        &audio,
        AudioCommand::SetPitchOffset {
            deck: id,
            semitones,
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

    // 波形が無いか、ビートグリッドが空（= BPM 未推定）なら、
    // バックグラウンドで解析を開始して両方を埋める。
    let beats_count = library
        .with_library(|lib| lib.load_beatgrid(track_id).map(|b| b.len()))
        .unwrap_or(0);
    let needs_analysis = result.is_none() || beats_count == 0;

    if needs_analysis && library.claim_analyzing(track_id) {
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
    let key_estimate = estimate_key(&audio);

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
    if let Some(k) = key_estimate {
        info!(
            key = %k.key.to_camelot(),
            r = k.correlation,
            margin = k.margin,
            "key estimated"
        );
        lib.save_track_key(track_id, k.key)
            .map_err(|e| e.to_string())?;
    }
    Ok(wf)
}

/// UI に渡すビート1拍分の DTO。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
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
pub fn get_resource_stats(stats: State<'_, SystemStatsHandle>) -> ResourceStats {
    stats.snapshot()
}

// ======== Audio devices ========

#[tauri::command]
pub fn list_audio_devices() -> Vec<String> {
    OutputDevice::list_available()
}

// ======== Settings ========

#[tauri::command]
pub fn get_settings(settings: State<'_, SettingsHandle>) -> AppSettings {
    settings.get()
}

#[tauri::command]
pub fn save_settings(
    settings: State<'_, SettingsHandle>,
    new_settings: AppSettings,
) -> Result<(), String> {
    settings.set(new_settings).map_err(|e| e.to_string())
}

// ======== Hot Cues ========

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct HotCueDto {
    pub slot: u8,
    pub position_sec: f64,
}

#[tauri::command]
pub fn list_hot_cues(
    library: State<'_, LibraryHandle>,
    track_id: String,
) -> Result<Vec<HotCueDto>, String> {
    let uuid = Uuid::parse_str(&track_id).map_err(|e| format!("invalid track id: {e}"))?;
    let id = TrackId::from_uuid(uuid);
    library.with_library(|lib| {
        lib.list_hot_cues(id)
            .map(|rows| {
                rows.into_iter()
                    .map(|(slot, position_sec)| HotCueDto { slot, position_sec })
                    .collect()
            })
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn set_hot_cue(
    library: State<'_, LibraryHandle>,
    track_id: String,
    slot: u8,
    position_sec: f64,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&track_id).map_err(|e| format!("invalid track id: {e}"))?;
    let id = TrackId::from_uuid(uuid);
    library.with_library(|lib| {
        lib.set_hot_cue(id, slot, position_sec)
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn delete_hot_cue(
    library: State<'_, LibraryHandle>,
    track_id: String,
    slot: u8,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&track_id).map_err(|e| format!("invalid track id: {e}"))?;
    let id = TrackId::from_uuid(uuid);
    library.with_library(|lib| {
        lib.delete_hot_cue(id, slot).map_err(|e| e.to_string())
    })
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

// ======== Templates (transition 自動オートメーション) ========

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TemplatePresetDto {
    pub id: String,
    pub name: String,
    pub duration_beats: f64,
}

#[tauri::command]
pub fn list_template_presets() -> Vec<TemplatePresetDto> {
    Template::all_presets()
        .into_iter()
        .map(|t| TemplatePresetDto {
            id: t.id,
            name: t.name,
            duration_beats: t.duration_beats,
        })
        .collect()
}

#[tauri::command]
pub fn get_template_preset(preset_id: String) -> Result<Template, String> {
    Template::all_presets()
        .into_iter()
        .find(|t| t.id == preset_id)
        .ok_or_else(|| format!("unknown preset: {preset_id}"))
}

#[tauri::command]
pub fn start_template_preset(
    audio: State<'_, AudioHandle>,
    preset_id: String,
    bpm: f32,
) -> CmdResult {
    let preset = Template::all_presets()
        .into_iter()
        .find(|t| t.id == preset_id)
        .ok_or_else(|| format!("unknown preset: {preset_id}"))?;
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(format!("invalid bpm: {bpm}"));
    }
    send(&audio, AudioCommand::StartTemplate { template: preset, bpm })
}

#[tauri::command]
pub fn abort_template(audio: State<'_, AudioHandle>) -> CmdResult {
    send(&audio, AudioCommand::AbortTemplate)
}

#[tauri::command]
pub fn override_param(
    audio: State<'_, AudioHandle>,
    target_key: String,
) -> CmdResult {
    let target = crate::audio_engine::key_to_target(&target_key)?;
    send(&audio, AudioCommand::OverrideParam { target })
}

#[tauri::command]
pub fn resume_param(
    audio: State<'_, AudioHandle>,
    target_key: String,
    duration_beats: Option<f64>,
) -> CmdResult {
    let target = crate::audio_engine::key_to_target(&target_key)?;
    let dur = duration_beats.unwrap_or(4.0).max(0.25);
    send(
        &audio,
        AudioCommand::ResumeParam {
            target,
            duration_beats: dur,
        },
    )
}

#[tauri::command]
pub fn commit_param(audio: State<'_, AudioHandle>, target_key: String) -> CmdResult {
    let target = crate::audio_engine::key_to_target(&target_key)?;
    send(&audio, AudioCommand::CommitParam { target })
}

// ======== Typed Cue (Cue editor / dynamic matching 用) ========

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CueDto {
    pub id: String,
    pub track_id: String,
    pub position_beats: f64,
    pub cue_type: String,
    pub bpm_at_cue: f32,
    pub key_camelot: String,
    pub energy_level: f32,
    pub phrase_length: u32,
    /// "entry" / "exit" の集合。
    pub mixable_as: Vec<String>,
    pub compatible_energy_min: f32,
    pub compatible_energy_max: f32,
    pub section_start_beats: Option<f64>,
    pub section_end_beats: Option<f64>,
}

impl From<&Cue> for CueDto {
    fn from(c: &Cue) -> Self {
        let (start, end) = match &c.section {
            Some(r) => (Some(r.start), Some(r.end)),
            None => (None, None),
        };
        Self {
            id: c.id.as_uuid().to_string(),
            track_id: c.track_id.as_uuid().to_string(),
            position_beats: c.position_beats,
            cue_type: cue_type_to_string(c.cue_type),
            bpm_at_cue: c.bpm_at_cue,
            key_camelot: c.key_at_cue.to_camelot(),
            energy_level: c.energy_level,
            phrase_length: c.phrase_length,
            mixable_as: c
                .mixable_as
                .iter()
                .map(|r| mix_role_to_string(*r).to_string())
                .collect(),
            compatible_energy_min: c.compatible_energy.start,
            compatible_energy_max: c.compatible_energy.end,
            section_start_beats: start,
            section_end_beats: end,
        }
    }
}

fn cue_type_to_string(t: CueType) -> String {
    match t {
        CueType::HotCue => "hot_cue",
        CueType::IntroStart => "intro_start",
        CueType::IntroEnd => "intro_end",
        CueType::Breakdown => "breakdown",
        CueType::Drop => "drop",
        CueType::Outro => "outro",
        CueType::CustomHotCue => "custom_hot_cue",
    }
    .to_string()
}

fn parse_cue_type(s: &str) -> Result<CueType, String> {
    Ok(match s {
        "hot_cue" => CueType::HotCue,
        "intro_start" => CueType::IntroStart,
        "intro_end" => CueType::IntroEnd,
        "breakdown" => CueType::Breakdown,
        "drop" => CueType::Drop,
        "outro" => CueType::Outro,
        "custom_hot_cue" => CueType::CustomHotCue,
        other => return Err(format!("invalid cue_type: {other}")),
    })
}

fn mix_role_to_string(r: MixRole) -> &'static str {
    match r {
        MixRole::Entry => "entry",
        MixRole::Exit => "exit",
    }
}

fn parse_mix_role(s: &str) -> Result<MixRole, String> {
    Ok(match s {
        "entry" => MixRole::Entry,
        "exit" => MixRole::Exit,
        other => return Err(format!("invalid mix role: {other}")),
    })
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct InsertCueArgs {
    pub track_id: String,
    pub position_beats: f64,
    pub cue_type: String,
    pub phrase_length: u32,
    /// 0..=1。空なら 0.5。
    pub energy_level: Option<f32>,
    /// `["entry"]` / `["exit"]` / `["entry", "exit"]`。
    pub mix_roles: Vec<String>,
    /// セクション Cue の場合の終端 (beats)。
    pub section_end_beats: Option<f64>,
}

/// `insert_cue` のコアロジック。Tauri command と HTTP API の両方から呼ばれる。
pub fn insert_cue_impl(library: &LibraryHandle, args: InsertCueArgs) -> Result<CueDto, String> {
    let uuid = Uuid::parse_str(&args.track_id).map_err(|e| format!("invalid track id: {e}"))?;
    let track_id = TrackId::from_uuid(uuid);
    let cue_type = parse_cue_type(&args.cue_type)?;
    let energy = args.energy_level.unwrap_or(0.5).clamp(0.0, 1.0);

    let track = library
        .with_library(|lib| lib.get_track(track_id).map_err(|e| e.to_string()))?
        .ok_or_else(|| format!("track not found: {}", args.track_id))?;
    // bpm == 0 の未解析トラックは Cue::new で弾かれる。
    let bpm = if track.bpm > 0.0 { track.bpm } else { 120.0 };

    let mut cue = Cue::new(
        track_id,
        args.position_beats,
        cue_type,
        bpm,
        track.key,
        energy,
        args.phrase_length,
    )
    .map_err(|e| e.to_string())?;

    let roles: Result<Vec<_>, _> = args.mix_roles.iter().map(|s| parse_mix_role(s)).collect();
    cue = cue.with_mix_roles(roles?);

    if let Some(end) = args.section_end_beats {
        cue = cue
            .with_section(args.position_beats..end)
            .map_err(|e| e.to_string())?;
    }

    library.with_library(|lib| lib.insert_cue(&cue).map_err(|e| e.to_string()))?;
    Ok(CueDto::from(&cue))
}

#[tauri::command]
pub fn insert_cue(
    library: State<'_, LibraryHandle>,
    args: InsertCueArgs,
) -> Result<CueDto, String> {
    insert_cue_impl(&library, args)
}

#[tauri::command]
pub fn list_cues(
    library: State<'_, LibraryHandle>,
    track_id: String,
) -> Result<Vec<CueDto>, String> {
    let uuid = Uuid::parse_str(&track_id).map_err(|e| format!("invalid track id: {e}"))?;
    let id = TrackId::from_uuid(uuid);
    library.with_library(|lib| {
        lib.list_cues_for_track(id)
            .map(|cs| cs.iter().map(CueDto::from).collect())
            .map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn delete_cue(
    library: State<'_, LibraryHandle>,
    cue_id: String,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&cue_id).map_err(|e| format!("invalid cue id: {e}"))?;
    let id = CueId::from_uuid(uuid);
    library.with_library(|lib| lib.delete_cue(id).map_err(|e| e.to_string()))
}

/// 開発・動作確認用: ライブラリ全 track にダミーの Drop @ Entry を 1 つずつ挿入する。
///
/// - 既に `Drop @ Entry` を持つ track はスキップ。
/// - 位置: 32 拍目 (= 約 1 小節 + 16 拍目あたりの典型的な Drop 位置)。
/// - 戻り値は新規挿入された Cue の数。
#[tauri::command]
pub fn inject_demo_cues(library: State<'_, LibraryHandle>) -> Result<usize, String> {
    let tracks = library
        .with_library(|lib| lib.list_tracks().map_err(|e| e.to_string()))?;
    let mut inserted = 0usize;
    for track in tracks {
        if track.bpm <= 0.0 {
            continue;
        }
        let existing = library
            .with_library(|lib| lib.list_cues_for_track(track.id).map_err(|e| e.to_string()))?;
        let already = existing
            .iter()
            .any(|c| c.cue_type == CueType::Drop && c.can_be(MixRole::Entry));
        if already {
            continue;
        }
        let cue = Cue::new(track.id, 32.0, CueType::Drop, track.bpm, track.key, 0.5, 32)
            .map_err(|e| e.to_string())?
            .with_mix_roles([MixRole::Entry]);
        library.with_library(|lib| lib.insert_cue(&cue).map_err(|e| e.to_string()))?;
        inserted += 1;
    }
    Ok(inserted)
}

// ======== Cue 動的マッチング (MixSuggestion 用) ========

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MatchCandidateDto {
    pub cue: CueDto,
    pub track: TrackSummary,
    pub bpm_score: f32,
    pub key_score: f32,
    pub energy_score: f32,
    pub overall_score: f32,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct MatchQueryArgs {
    pub bpm: f32,
    /// Camelot 記法 ("8A" / "11B" 等)。
    pub key_camelot: String,
    pub energy: f32,
    pub max_bpm_diff: Option<f32>,
    pub limit: Option<u32>,
    /// このトラックに属する Cue は除外 (= 自分自身を提案しない)。
    pub exclude_track_id: Option<String>,
}

pub fn list_match_candidates_impl(
    library: &LibraryHandle,
    args: MatchQueryArgs,
) -> Result<Vec<MatchCandidateDto>, String> {
    let key: conduction_core::Key = args
        .key_camelot
        .parse()
        .map_err(|e: conduction_core::CoreError| e.to_string())?;
    let mut query = conduction_conductor::MatchQuery::new(args.bpm, key, args.energy);
    if let Some(d) = args.max_bpm_diff {
        query.max_bpm_diff = d;
    }
    let limit = args.limit.unwrap_or(8) as usize;

    let pool = library
        .with_library(|lib| lib.list_all_cues_with_tracks().map_err(|e| e.to_string()))?;

    let exclude = args
        .exclude_track_id
        .as_deref()
        .map(|s| {
            Uuid::parse_str(s)
                .map(TrackId::from_uuid)
                .map_err(|e| format!("invalid exclude_track_id: {e}"))
        })
        .transpose()?;
    let pool: Vec<(Cue, conduction_core::Track)> = pool
        .into_iter()
        .filter(|(c, _)| Some(c.track_id) != exclude)
        .collect();

    let candidates = conduction_conductor::find_candidates(&query, &pool, limit);
    let out = candidates
        .into_iter()
        .map(|sc| MatchCandidateDto {
            cue: CueDto::from(sc.cue),
            track: TrackSummary::from_track(sc.track),
            bpm_score: sc.score.bpm_score,
            key_score: sc.score.key_score,
            energy_score: sc.score.energy_score,
            overall_score: sc.score.overall,
        })
        .collect();
    Ok(out)
}

#[tauri::command]
pub fn list_match_candidates(
    library: State<'_, LibraryHandle>,
    args: MatchQueryArgs,
) -> Result<Vec<MatchCandidateDto>, String> {
    list_match_candidates_impl(&library, args)
}

// ======== USB Export (rekordbox-compatible) ========

#[tauri::command]
pub fn export_preview(
    library: State<'_, LibraryHandle>,
    destination: String,
) -> Result<conduction_export::ExportPreview, String> {
    let dest = PathBuf::from(destination);
    let plan = library.with_library(|lib| {
        conduction_export::build_plan(lib, dest).map_err(|e| e.to_string())
    })?;
    Ok(conduction_export::ExportPreview::from_plan(&plan))
}

#[tauri::command]
pub fn export_execute(
    library: State<'_, LibraryHandle>,
    destination: String,
) -> Result<conduction_export::ExportReport, String> {
    let dest = PathBuf::from(destination);
    let plan = library.with_library(|lib| {
        conduction_export::build_plan(lib, dest).map_err(|e| e.to_string())
    })?;
    conduction_export::execute(&plan).map_err(|e| e.to_string())
}

// ======== YouTube ========

#[tauri::command]
pub fn yt_dlp_available() -> bool {
    crate::youtube::is_available()
}

#[tauri::command]
pub fn yt_search(
    query: String,
    limit: u32,
) -> Result<Vec<conduction_download::VideoSearchResult>, String> {
    crate::youtube::search(&query, limit as usize)
}

#[tauri::command]
pub fn yt_download(
    app: AppHandle,
    library: State<'_, LibraryHandle>,
    url: String,
    format: String,
    request_id: String,
) -> Result<TrackSummary, String> {
    let format = crate::youtube::parse_format(&format)?;
    let app_for_emit = app.clone();
    let req_for_emit = request_id.clone();
    let result = crate::youtube::download_and_import(&library, &url, format, move |ev| {
        let _ = app_for_emit.emit(
            "yt:progress",
            serde_json::json!({
                "request_id": req_for_emit,
                "raw": ev.raw,
                "percent": ev.percent,
                "eta_sec": ev.eta_sec,
                "stage": ev.stage,
            }),
        );
    });
    let _ = app.emit(
        "yt:done",
        serde_json::json!({
            "request_id": request_id,
            "ok": result.is_ok(),
        }),
    );
    result
}
