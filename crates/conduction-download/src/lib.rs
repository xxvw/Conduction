//! conduction-download — yt-dlp wrapper.
//!
//! `yt-dlp` がシステムにインストールされている時のみ実行可能。
//! `is_available()` で検出できる。検出は OS の PATH 解決に任せる。

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("yt-dlp is not available on PATH")]
    NotInstalled,
    #[error("yt-dlp exited with status {code:?}: {stderr}")]
    Failed { code: Option<i32>, stderr: String },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no output path emitted by yt-dlp")]
    NoOutputPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct VideoSearchResult {
    pub id: String,
    pub title: String,
    pub url: String,
    pub channel: String,
    pub duration_sec: Option<f64>,
    pub thumbnail: Option<String>,
    pub view_count: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    M4a,
    Mp3,
    Opus,
    Wav,
    Flac,
}

impl AudioFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::M4a => "m4a",
            Self::Mp3 => "mp3",
            Self::Opus => "opus",
            Self::Wav => "wav",
            Self::Flac => "flac",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "m4a" => Some(Self::M4a),
            "mp3" => Some(Self::Mp3),
            "opus" => Some(Self::Opus),
            "wav" => Some(Self::Wav),
            "flac" => Some(Self::Flac),
            _ => None,
        }
    }
}

/// `yt-dlp --version` を試行して終了コードで判定。
pub fn is_available() -> bool {
    Command::new("yt-dlp")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// `ytsearchN:<query>` で N 件の動画メタデータを取得。
///
/// yt-dlp は `--dump-json` で 1 行 1 動画の JSON を stdout に出す。
pub fn search(query: &str, limit: usize) -> Result<Vec<VideoSearchResult>, DownloadError> {
    if !is_available() {
        return Err(DownloadError::NotInstalled);
    }
    let limit = limit.clamp(1, 50);
    let target = format!("ytsearch{limit}:{query}");
    debug!(query, limit, "yt-dlp search");
    let output = Command::new("yt-dlp")
        .args([
            target.as_str(),
            "--dump-json",
            "--no-playlist",
            "--skip-download",
            "--no-warnings",
            "--ignore-errors",
            "--flat-playlist",
        ])
        .output()?;

    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(DownloadError::Failed {
            code: output.status.code(),
            stderr,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::with_capacity(limit);
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(v) => results.push(parse_search_entry(&v)),
            Err(e) => warn!(error = %e, "skipped unparseable yt-dlp line"),
        }
    }
    Ok(results)
}

fn parse_search_entry(v: &serde_json::Value) -> VideoSearchResult {
    let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let url = v
        .get("webpage_url")
        .and_then(|x| x.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            if id.is_empty() {
                String::new()
            } else {
                format!("https://www.youtube.com/watch?v={id}")
            }
        });
    let channel = v
        .get("channel")
        .and_then(|x| x.as_str())
        .or_else(|| v.get("uploader").and_then(|x| x.as_str()))
        .unwrap_or("")
        .to_string();
    let duration_sec = v.get("duration").and_then(|x| x.as_f64());
    let thumbnail = v
        .get("thumbnail")
        .and_then(|x| x.as_str())
        .map(String::from);
    let view_count = v.get("view_count").and_then(|x| x.as_u64());
    VideoSearchResult {
        id,
        title,
        url,
        channel,
        duration_sec,
        thumbnail,
        view_count,
    }
}

/// 単一の動画 URL を音声トラックとしてダウンロードする。
///
/// 出力先は `out_dir/<title> [<id>].<ext>`。
/// 戻り値は実際の出力ファイル絶対パス。
pub fn download(
    url: &str,
    format: AudioFormat,
    out_dir: &Path,
) -> Result<PathBuf, DownloadError> {
    if !is_available() {
        return Err(DownloadError::NotInstalled);
    }
    std::fs::create_dir_all(out_dir)?;

    let template = out_dir.join("%(title).200B [%(id)s].%(ext)s");
    let template_str = template.to_string_lossy();
    let format_str = format.as_str();

    debug!(url, format = format_str, dir = %out_dir.display(), "yt-dlp download");

    let output = Command::new("yt-dlp")
        .args([
            "-x",
            "--audio-format",
            format_str,
            "--no-playlist",
            "--no-warnings",
            "-o",
            template_str.as_ref(),
            "--print",
            "after_move:filepath",
            url,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(DownloadError::Failed {
            code: output.status.code(),
            stderr,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .last()
        .ok_or(DownloadError::NoOutputPath)?;
    Ok(PathBuf::from(path))
}
