//! YouTube 検索 & ダウンロード共通ロジック。
//!
//! `yt-dlp` を子プロセスで spawn して検索・音声ダウンロードを行い、
//! 完了したファイルを Library に import → バックグラウンド解析まで実施する。

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use conduction_analysis::{
    decode_to_pcm, estimate_beatgrid, generate_waveform, DEFAULT_WAVEFORM_BINS,
};
use conduction_core::TrackId;
use conduction_download::{search as yt_search, AudioFormat, ProgressEvent, VideoSearchResult};
use conduction_library::{build_track_from_file, Library};
use directories::ProjectDirs;
use parking_lot::Mutex;
use tracing::{info, warn};

use crate::library_state::{LibraryHandle, TrackSummary};

pub fn is_available() -> bool {
    conduction_download::is_available()
}

pub fn search(query: &str, limit: usize) -> Result<Vec<VideoSearchResult>, String> {
    yt_search(query, limit).map_err(|e| e.to_string())
}

pub fn parse_format(s: &str) -> Result<AudioFormat, String> {
    AudioFormat::parse(s).ok_or_else(|| format!("invalid audio format: {s}"))
}

/// `~/Library/Application Support/com.xxvw.conduction/downloads/` (OS規約準拠) を返す。
pub fn download_dir() -> Result<PathBuf, String> {
    let dirs = ProjectDirs::from("com", "xxvw", "Conduction")
        .ok_or_else(|| "no user data directory available from OS".to_string())?;
    Ok(dirs.data_dir().join("downloads"))
}

/// URL を音声でダウンロードして Library に登録、解析をバックグラウンド起動する。
/// 成功時は登録済の `TrackSummary` を返す。`on_progress` は yt-dlp の進捗
/// コールバック (UI への emit などに使う)。
pub fn download_and_import<F>(
    library: &LibraryHandle,
    url: &str,
    format: AudioFormat,
    on_progress: F,
) -> Result<TrackSummary, String>
where
    F: FnMut(&ProgressEvent),
{
    let dir = download_dir()?;
    info!(url, format = format.as_str(), dir = %dir.display(), "yt download starting");
    let path = conduction_download::download(url, format, &dir, on_progress)
        .map_err(|e| e.to_string())?;
    info!(path = %path.display(), "yt download completed");

    let track = build_track_from_file(&path).map_err(|e| e.to_string())?;
    let stored = library.with_library(|lib| -> Result<_, String> {
        let id = lib.upsert_track_by_path(&track).map_err(|e| e.to_string())?;
        let stored = lib
            .get_track(id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "track disappeared after insert".to_string())?;
        Ok(stored)
    })?;

    let needs_waveform = library
        .with_library(|lib| lib.load_waveform(stored.id).map(|w| w.is_none()))
        .unwrap_or(true);
    if needs_waveform {
        spawn_background_analyze(library.shared(), stored.id, stored.path.clone());
    }
    Ok(TrackSummary::from_track(&stored))
}

fn spawn_background_analyze(
    library: Arc<Mutex<Library>>,
    track_id: TrackId,
    path: PathBuf,
) {
    let _ = thread::Builder::new()
        .name("yt-analyze".into())
        .spawn(move || {
            let started = std::time::Instant::now();
            match analyze_internal(&library, track_id, &path) {
                Ok(_) => info!(
                    path = %path.display(),
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "yt-import analyze completed"
                ),
                Err(e) => warn!(
                    error = %e,
                    path = %path.display(),
                    "yt-import analyze failed"
                ),
            }
        });
}

fn analyze_internal(
    library: &Arc<Mutex<Library>>,
    track_id: TrackId,
    path: &std::path::Path,
) -> Result<(), String> {
    let audio = decode_to_pcm(path).map_err(|e| e.to_string())?;
    let total_sec = audio.duration_sec();
    let wf = generate_waveform(&audio, DEFAULT_WAVEFORM_BINS);
    let estimate = estimate_beatgrid(&audio);
    let mut lib = library.lock();
    lib.save_waveform(track_id, &wf).map_err(|e| e.to_string())?;
    if let Some(est) = estimate {
        let beats = est.beats(total_sec);
        lib.save_track_analysis(track_id, est.bpm, &beats)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
