//! conduction-export — rekordbox 互換 USB を生成する。
//!
//! Phase 1 (現在): プラン構築 + 実行 stub のスケルトン。
//! Phase 2 以降で `rekordcrate` の BinWrite プリミティブを使って実際に
//! `export.pdb` / `.DAT` / `.EXT` を書き出す。
//!
//! 設計方針:
//! - Library DB から全 track をスナップショットして `ExportPlan` に組み立てる
//!   (この段階では USB に触れない、純粋なメモリ上のプラン).
//! - `execute(&plan)` で実際にファイルシステム操作を走らせる
//!   (Phase 2 以降の実装範囲).

#![forbid(unsafe_code)]

pub mod api;
pub mod format;
pub use api::{
    ConflictStrategy, ExportOptions, Exporter, ImportOptions, Importer, LibraryExportReport,
    LibraryImportReport,
};
pub use format::{Format, FormatInfo, TargetKind};

use std::path::PathBuf;

use conduction_analysis::WaveformPreview;
use conduction_core::{Beat, Track};
use conduction_library::Library;
use serde::Serialize;
use thiserror::Error;
use tracing::{debug, info};

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("export writer not implemented yet (Phase 1 skeleton)")]
    NotImplemented,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("library: {0}")]
    Library(String),
    #[error("destination is not a directory: {0}")]
    InvalidDestination(PathBuf),
}

/// 1 trk 分の export 入力データ。Library DB に存在する全情報を集約する。
#[derive(Debug, Clone, Serialize)]
pub struct ExportTrack {
    pub track: Track,
    pub beats: Vec<Beat>,
    pub waveform: Option<WaveformPreview>,
    /// `(slot, position_sec)`。slot は 1..=8。
    pub hot_cues: Vec<(u8, f64)>,
}

/// USB に書き出すプラン全体。`root` は USB の絶対パス。
#[derive(Debug, Clone, Serialize)]
pub struct ExportPlan {
    pub root: PathBuf,
    pub tracks: Vec<ExportTrack>,
}

impl ExportPlan {
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// 元音源の総バイト数 (USB 容量見積もり)。失敗したファイルは 0 換算。
    pub fn estimate_audio_bytes(&self) -> u64 {
        self.tracks
            .iter()
            .map(|t| {
                std::fs::metadata(&t.track.path)
                    .map(|m| m.len())
                    .unwrap_or(0)
            })
            .sum()
    }
}

/// プラン構築時に返す UI 向け要約 (件数・容量・ビート/Hot Cue サマリ)。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ExportPreview {
    pub root: String,
    pub track_count: usize,
    pub estimated_audio_bytes: u64,
    pub tracks_with_beatgrid: usize,
    pub tracks_with_waveform: usize,
    pub total_hot_cues: usize,
}

impl ExportPreview {
    pub fn from_plan(plan: &ExportPlan) -> Self {
        let tracks_with_beatgrid = plan.tracks.iter().filter(|t| !t.beats.is_empty()).count();
        let tracks_with_waveform = plan.tracks.iter().filter(|t| t.waveform.is_some()).count();
        let total_hot_cues = plan.tracks.iter().map(|t| t.hot_cues.len()).sum();
        Self {
            root: plan.root.display().to_string(),
            track_count: plan.tracks.len(),
            estimated_audio_bytes: plan.estimate_audio_bytes(),
            tracks_with_beatgrid,
            tracks_with_waveform,
            total_hot_cues,
        }
    }
}

/// 実行後の結果サマリ (Phase 2 以降)。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ExportReport {
    pub tracks_written: usize,
    pub bytes_written: u64,
}

/// Library DB から全 track + 解析結果を集約してプランを組み立てる。
///
/// 注意: ここでは USB ディレクトリにはまだ触らない。`root` は execute 段で使う。
pub fn build_plan(library: &Library, root: PathBuf) -> Result<ExportPlan, ExportError> {
    let tracks = library
        .list_tracks()
        .map_err(|e| ExportError::Library(e.to_string()))?;

    debug!(track_count = tracks.len(), "export plan: gathering analysis");

    let mut planned: Vec<ExportTrack> = Vec::with_capacity(tracks.len());
    for t in tracks {
        let beats = library
            .load_beatgrid(t.id)
            .map_err(|e| ExportError::Library(e.to_string()))?;
        let waveform = library
            .load_waveform(t.id)
            .map_err(|e| ExportError::Library(e.to_string()))?;
        let hot_cues = library
            .list_hot_cues(t.id)
            .map_err(|e| ExportError::Library(e.to_string()))?;
        planned.push(ExportTrack {
            track: t,
            beats,
            waveform,
            hot_cues,
        });
    }

    info!(
        root = %root.display(),
        tracks = planned.len(),
        "export plan built"
    );
    Ok(ExportPlan {
        root,
        tracks: planned,
    })
}

/// プランを実行して USB に書き出す。Phase 2 以降で実装。
pub fn execute(plan: &ExportPlan) -> Result<ExportReport, ExportError> {
    if !plan.root.is_dir() {
        return Err(ExportError::InvalidDestination(plan.root.clone()));
    }
    Err(ExportError::NotImplemented)
}
