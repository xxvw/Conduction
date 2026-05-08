//! Library state shared across Tauri command invocations.
//!
//! rusqlite::Connection は `!Sync` のため、`Arc<Mutex<Library>>` で包む。
//! ライブラリ操作はコマンドごとに短時間のロックで済むため、パフォーマンス上も問題ない。

use std::collections::HashSet;
use std::sync::Arc;

use conduction_core::{Track, TrackId};
use conduction_library::Library;
use parking_lot::Mutex;
use serde::Serialize;
use tracing::info;

/// Tauri State として共有されるライブラリハンドル。
pub struct LibraryHandle {
    inner: Arc<Mutex<Library>>,
    /// 現在バックグラウンド解析が走っている TrackId の集合。
    /// `get_waveform` polling 中の重複起動を防止する。
    analyzing: Arc<Mutex<HashSet<TrackId>>>,
}

impl LibraryHandle {
    /// OS 規約に従って `library.db` を開く。
    pub fn open_default() -> anyhow::Result<Self> {
        let lib = Library::open_default()?;
        info!("library opened at OS default location");
        Ok(Self {
            inner: Arc::new(Mutex::new(lib)),
            analyzing: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub fn with_library<T>(&self, f: impl FnOnce(&mut Library) -> T) -> T {
        let mut guard = self.inner.lock();
        f(&mut guard)
    }

    /// バックグラウンドスレッドで使うために `Arc` を複製する。
    pub fn shared(&self) -> Arc<Mutex<Library>> {
        self.inner.clone()
    }

    /// 指定 track の解析を開始可能なら `true`、既に走っているなら `false` を返す。
    /// `true` を返した呼び出し側は、解析完了後に `release_analyzing` を呼ぶ責務を持つ。
    pub fn claim_analyzing(&self, track_id: TrackId) -> bool {
        self.analyzing.lock().insert(track_id)
    }

    pub fn release_analyzing(&self, track_id: TrackId) {
        self.analyzing.lock().remove(&track_id);
    }

    /// バックグラウンドスレッドが完了時に release できるよう、フラグ集合の Arc を返す。
    pub fn analyzing_set(&self) -> Arc<Mutex<HashSet<TrackId>>> {
        self.analyzing.clone()
    }
}

/// UI に返すトラック要約。Key は Camelot 文字列（"8A" など）に整形済み。
#[derive(Debug, Clone, Serialize)]
pub struct TrackSummary {
    pub id: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub duration_sec: f64,
    pub bpm: f32,
    pub key: String,
    pub energy: f32,
    pub beatgrid_verified: bool,
    pub analyzed: bool,
}

impl TrackSummary {
    pub fn from_track(t: &Track) -> Self {
        Self {
            id: t.id.as_uuid().to_string(),
            path: t.path.to_string_lossy().to_string(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            genre: t.genre.clone(),
            duration_sec: t.duration.as_secs_f64(),
            bpm: t.bpm,
            key: t.key.to_camelot(),
            energy: t.energy,
            beatgrid_verified: t.beatgrid_verified,
            analyzed: t.analyzed_at.is_some(),
        }
    }
}
