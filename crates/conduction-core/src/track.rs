use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::beat::Beat;
use crate::cue::Cue;
use crate::error::{CoreError, CoreResult};
use crate::ids::TrackId;
use crate::key::Key;

/// 曲 1 本分のメタデータ。要件 11 §Track に準拠。
///
/// 楽曲解析結果は `analyzed_at` が `Some` であることをもって整合的とみなす。
/// ビートグリッドの手動検証完了は `beatgrid_verified` で別フラグ管理する
/// （要件 6.4: 検証完了までライブラリで「未検証」扱い）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub path: PathBuf,

    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,

    #[serde(with = "duration_serde")]
    pub duration: Duration,

    pub bpm: f32,
    pub key: Key,
    /// 0.0 〜 1.0。
    pub energy: f32,

    pub cues: Vec<Cue>,
    pub beatgrid: Vec<Beat>,
    /// ビートグリッドが手動検証済みかどうか。未検証曲はライブ再生から除外する想定。
    pub beatgrid_verified: bool,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub analyzed_at: Option<DateTime<Utc>>,
}

impl Track {
    /// 解析前のプレースホルダ生成。ライブラリに曲を追加した直後に使う。
    pub fn placeholder(path: PathBuf, key: Key) -> Self {
        Self {
            id: TrackId::new(),
            path,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
            duration: Duration::ZERO,
            bpm: 0.0,
            key,
            energy: 0.0,
            cues: Vec::new(),
            beatgrid: Vec::new(),
            beatgrid_verified: false,
            analyzed_at: None,
        }
    }

    /// BPM / エネルギーの範囲チェック。
    pub fn validate(&self) -> CoreResult<()> {
        if !self.bpm.is_finite() || self.bpm < 0.0 {
            return Err(CoreError::OutOfRange {
                field: "bpm",
                value: self.bpm as f64,
                expected: ">= 0 and finite",
            });
        }
        if !(0.0..=1.0).contains(&self.energy) {
            return Err(CoreError::OutOfRange {
                field: "energy",
                value: self.energy as f64,
                expected: "0.0 ..= 1.0",
            });
        }
        Ok(())
    }

    /// ビートグリッドの検証フラグを立てる（手動補正完了時に呼ばれる）。
    pub fn mark_beatgrid_verified(&mut self) {
        self.beatgrid_verified = true;
    }

    /// ライブ再生として安全か（= 解析済み かつ ビートグリッド検証済み）。
    pub fn is_ready_for_live(&self) -> bool {
        self.analyzed_at.is_some() && self.beatgrid_verified
    }
}

/// `Duration` を秒（f64）で JSON シリアライズ。TOML でも読みやすい形を優先。
mod duration_serde {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(d.as_secs_f64())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = f64::deserialize(d)?;
        if !(secs.is_finite() && secs >= 0.0) {
            return Err(serde::de::Error::custom("duration seconds must be >= 0"));
        }
        Ok(Duration::from_secs_f64(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::KeyMode;

    fn sample_key() -> Key {
        Key::new(8, KeyMode::Minor).unwrap()
    }

    #[test]
    fn placeholder_is_not_live_ready() {
        let track = Track::placeholder(PathBuf::from("/tmp/x.mp3"), sample_key());
        assert!(!track.is_ready_for_live());
    }

    #[test]
    fn becomes_live_ready_after_analysis_and_verification() {
        let mut track = Track::placeholder(PathBuf::from("/tmp/x.mp3"), sample_key());
        track.analyzed_at = Some(Utc::now());
        track.mark_beatgrid_verified();
        assert!(track.is_ready_for_live());
    }

    #[test]
    fn validate_rejects_bad_energy() {
        let mut track = Track::placeholder(PathBuf::from("/tmp/x.mp3"), sample_key());
        track.energy = 2.0;
        assert!(track.validate().is_err());
    }

    #[test]
    fn serde_roundtrip_preserves_duration() {
        let mut track = Track::placeholder(PathBuf::from("/tmp/x.mp3"), sample_key());
        track.duration = Duration::from_secs_f64(318.5);
        track.bpm = 128.0;
        track.energy = 0.7;
        let json = serde_json::to_string(&track).unwrap();
        let back: Track = serde_json::from_str(&json).unwrap();
        assert_eq!(track, back);
    }
}
