//! conduction-download — yt-dlp wrapper.
//!
//! `yt-dlp` がシステムにインストールされている時のみ実行可能。
//! `is_available()` で検出できる。検出は OS の PATH 解決に任せる。

#![forbid(unsafe_code)]

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// yt-dlp の進捗 1 行分。`raw` は yt-dlp の出力をそのまま、`percent` は
/// `[download] XX.X%` をパースして数値化したもの (取れなければ `None`)。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ProgressEvent {
    pub raw: String,
    pub percent: Option<f32>,
    pub eta_sec: Option<f64>,
    pub stage: Stage,
}

#[derive(Debug, Clone, Copy, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Download,
    Postprocess,
    Other,
}

/// 単一の動画 URL を音声トラックとしてダウンロードする。
///
/// 出力先は `out_dir/<title> [<id>].<ext>`。
/// 戻り値は実際の出力ファイル絶対パス。`on_progress` は yt-dlp の stdout を
/// 改行ごとにパースして呼ばれる (UI への進捗通知に使う)。
pub fn download<F>(
    url: &str,
    format: AudioFormat,
    out_dir: &Path,
    mut on_progress: F,
) -> Result<PathBuf, DownloadError>
where
    F: FnMut(&ProgressEvent),
{
    if !is_available() {
        return Err(DownloadError::NotInstalled);
    }
    std::fs::create_dir_all(out_dir)?;

    let template = out_dir.join("%(title).200B [%(id)s].%(ext)s");
    let template_str = template.to_string_lossy();
    let format_str = format.as_str();

    debug!(url, format = format_str, dir = %out_dir.display(), "yt-dlp download");

    let mut child = Command::new("yt-dlp")
        .args([
            "-x",
            "--audio-format",
            format_str,
            "--no-playlist",
            "--no-warnings",
            "--newline",
            "-o",
            template_str.as_ref(),
            "--print",
            "after_move:filepath",
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().ok_or_else(|| {
        DownloadError::Failed {
            code: None,
            stderr: "failed to capture stdout".into(),
        }
    })?;
    let stderr = child.stderr.take();
    let stderr_handle = stderr.map(|mut s| {
        std::thread::spawn(move || {
            let mut buf = String::new();
            s.read_to_string(&mut buf).ok();
            buf
        })
    });

    let reader = BufReader::new(stdout);
    let mut last_path: Option<String> = None;
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                warn!(error = %e, "yt-dlp stdout read");
                break;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // `--print after_move:filepath` の出力 (絶対パス) を覚えておく。
        if Path::new(trimmed).is_absolute() && Path::new(trimmed).extension().is_some() {
            last_path = Some(trimmed.to_string());
        }
        let ev = parse_progress_line(&line);
        on_progress(&ev);
    }

    let status = child.wait()?;
    let stderr_text = stderr_handle
        .and_then(|h| h.join().ok())
        .unwrap_or_default();
    if !status.success() {
        return Err(DownloadError::Failed {
            code: status.code(),
            stderr: stderr_text,
        });
    }

    let path = last_path.ok_or(DownloadError::NoOutputPath)?;
    Ok(PathBuf::from(path))
}

fn parse_progress_line(line: &str) -> ProgressEvent {
    let stage = if line.starts_with("[download]") {
        Stage::Download
    } else if line.starts_with("[ExtractAudio]")
        || line.starts_with("[Metadata]")
        || line.starts_with("[FixupM4a]")
        || line.starts_with("[Fixup")
        || line.starts_with("[ffmpeg]")
    {
        Stage::Postprocess
    } else {
        Stage::Other
    };
    let percent = extract_percent(line);
    let eta_sec = extract_eta(line);
    ProgressEvent {
        raw: line.to_string(),
        percent,
        eta_sec,
        stage,
    }
}

fn extract_percent(line: &str) -> Option<f32> {
    // 例: "[download]   3.5% of  4.20MiB at ..."
    let pct_pos = line.find('%')?;
    let prefix = &line[..pct_pos];
    let num_start = prefix
        .rfind(|c: char| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);
    let num_str = &prefix[num_start..];
    num_str.trim().parse::<f32>().ok()
}

fn extract_eta(line: &str) -> Option<f64> {
    // 例: "ETA 00:03" または "ETA 1:23:45"
    let idx = line.find("ETA ")?;
    let rest = &line[idx + 4..];
    let token = rest.split_whitespace().next()?;
    let parts: Vec<&str> = token.split(':').collect();
    let secs = match parts.as_slice() {
        [m, s] => m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?,
        [h, m, s] => {
            h.parse::<f64>().ok()? * 3600.0
                + m.parse::<f64>().ok()? * 60.0
                + s.parse::<f64>().ok()?
        }
        _ => return None,
    };
    Some(secs)
}
