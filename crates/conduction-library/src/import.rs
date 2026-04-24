//! 楽曲ファイルからメタデータを抽出し、[`Track`] のプレースホルダを埋める。
//!
//! lofty 0.24 を用いて MP3 / FLAC / WAV / OGG / M4A のタグを読み取る。
//! BPM / キー / エネルギー等は解析パイプライン（Phase 3b）で埋める。

use std::path::Path;
use std::time::Duration;

use conduction_core::{Key, KeyMode, Track};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use tracing::{debug, warn};

use crate::error::{LibraryError, LibraryResult};

/// ファイルから抽出した生のメタデータ。
#[derive(Debug, Clone, Default)]
pub struct ImportedMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub duration: Duration,
}

/// 音声ファイルから ImportedMetadata を取り出す。
///
/// タグがない場合は空文字列で埋め、duration のみ properties から取得する。
pub fn extract_metadata(path: &Path) -> LibraryResult<ImportedMetadata> {
    let tagged = Probe::open(path)
        .and_then(|p| p.read())
        .map_err(|source| LibraryError::Metadata {
            path: path.to_path_buf(),
            source,
        })?;

    let duration = tagged.properties().duration();

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let meta = if let Some(tag) = tag {
        ImportedMetadata {
            title: tag.title().map(|s| s.to_string()).unwrap_or_default(),
            artist: tag.artist().map(|s| s.to_string()).unwrap_or_default(),
            album: tag.album().map(|s| s.to_string()).unwrap_or_default(),
            genre: tag.genre().map(|s| s.to_string()).unwrap_or_default(),
            duration,
        }
    } else {
        warn!(path = %path.display(), "no tags found");
        ImportedMetadata {
            duration,
            ..Default::default()
        }
    };

    debug!(
        path = %path.display(),
        title = %meta.title,
        artist = %meta.artist,
        duration_sec = meta.duration.as_secs_f64(),
        "metadata extracted",
    );
    Ok(meta)
}

/// ファイルパスから [`Track`] を構築する。
/// BPM / Key 等は placeholder の値（Key=`1A`、BPM=0）のまま。
pub fn build_track_from_file(path: &Path) -> LibraryResult<Track> {
    let meta = extract_metadata(path)?;
    // タグに title が無ければファイル名（拡張子なし）をフォールバック。
    let fallback_title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let default_key = Key::new(1, KeyMode::Minor).expect("1A is a valid Camelot key");
    let mut track = Track::placeholder(path.to_path_buf(), default_key);
    track.title = if meta.title.is_empty() {
        fallback_title
    } else {
        meta.title
    };
    track.artist = meta.artist;
    track.album = meta.album;
    track.genre = meta.genre;
    track.duration = meta.duration;
    Ok(track)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// 実ファイルに依存するので ignore 指定。ローカルで `cargo test -- --ignored import`
    /// を走らせて手元の wekapipo.mp3 を検証する想定。
    #[test]
    #[ignore = "requires ~/Downloads/wekapipo.mp3"]
    fn extract_real_mp3_metadata() {
        let home = std::env::var("HOME").unwrap();
        let path = PathBuf::from(format!("{home}/Downloads/wekapipo.mp3"));
        if !path.exists() {
            eprintln!("skipping: {} not found", path.display());
            return;
        }

        let meta = extract_metadata(&path).expect("should extract");
        assert!(meta.duration.as_secs() > 0);
    }

    #[test]
    fn missing_file_returns_metadata_error() {
        let err = extract_metadata(Path::new("/definitely/does/not/exist.mp3")).unwrap_err();
        assert!(matches!(err, LibraryError::Metadata { .. }));
    }
}
