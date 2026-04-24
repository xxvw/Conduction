use std::collections::BTreeSet;
use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::ids::{CueId, TrackId};
use crate::key::Key;

/// Cue の種別。要件 6.3。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CueType {
    /// 基本の Hot Cue（位置マーカー）。
    HotCue,
    /// イントロ開始。
    IntroStart,
    /// イントロ終了。
    IntroEnd,
    /// ブレイクダウン。
    Breakdown,
    /// ドロップ。
    Drop,
    /// アウトロ。
    Outro,
    /// ユーザー定義のタイプ付き Hot Cue。
    CustomHotCue,
}

/// 繋ぎでの Cue の役割。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum MixRole {
    /// 入口として使える（= 次曲の遷移開始点）。
    Entry,
    /// 出口として使える（= 前曲の遷移終了点）。
    Exit,
}

/// 楽曲内の Cue ポイント。位置マーカー + 構造的メタデータ + 繋ぎ用メタ。
///
/// 要件 11 データモデル概要 §Cue に準拠。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub id: CueId,
    pub track_id: TrackId,

    /// 拍数単位の位置（曲頭を 0 とする）。
    pub position_beats: f64,

    pub cue_type: CueType,

    /// セクション Cue の場合、開始点 + 範囲（拍数）。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub section: Option<Range<f64>>,

    // --- マッチング用メタデータ ---
    pub bpm_at_cue: f32,
    pub key_at_cue: Key,
    /// 0.0 〜 1.0。
    pub energy_level: f32,
    /// 16 / 32 / 64 小節など。
    pub phrase_length: u32,

    // --- 繋ぎ設定 ---
    /// この Cue が Entry / Exit として使えるか。`BTreeSet` で両方持ちを自然に表現。
    pub mixable_as: BTreeSet<MixRole>,
    /// Cue 動的マッチング時に許容するエネルギー範囲。
    pub compatible_energy: Range<f32>,
}

impl Cue {
    /// バリデーション込みのコンストラクタ。
    pub fn new(
        track_id: TrackId,
        position_beats: f64,
        cue_type: CueType,
        bpm_at_cue: f32,
        key_at_cue: Key,
        energy_level: f32,
        phrase_length: u32,
    ) -> CoreResult<Self> {
        if !position_beats.is_finite() || position_beats < 0.0 {
            return Err(CoreError::OutOfRange {
                field: "position_beats",
                value: position_beats,
                expected: ">= 0 and finite",
            });
        }
        if !(0.0..=1.0).contains(&energy_level) {
            return Err(CoreError::OutOfRange {
                field: "energy_level",
                value: energy_level as f64,
                expected: "0.0 ..= 1.0",
            });
        }
        if !bpm_at_cue.is_finite() || bpm_at_cue <= 0.0 {
            return Err(CoreError::OutOfRange {
                field: "bpm_at_cue",
                value: bpm_at_cue as f64,
                expected: "> 0 and finite",
            });
        }

        Ok(Self {
            id: CueId::new(),
            track_id,
            position_beats,
            cue_type,
            section: None,
            bpm_at_cue,
            key_at_cue,
            energy_level,
            phrase_length,
            mixable_as: BTreeSet::new(),
            compatible_energy: (energy_level - 0.2).max(0.0)..(energy_level + 0.2).min(1.0),
        })
    }

    /// セクション（開始 + 範囲）を付ける。`end > start` でなければエラー。
    pub fn with_section(mut self, range: Range<f64>) -> CoreResult<Self> {
        if !(range.start < range.end) {
            return Err(CoreError::InvalidRange {
                start: range.start,
                end: range.end,
            });
        }
        self.section = Some(range);
        Ok(self)
    }

    /// 繋ぎロールを設定する。`Entry` / `Exit` の重複はセットが吸収する。
    pub fn with_mix_roles<I: IntoIterator<Item = MixRole>>(mut self, roles: I) -> Self {
        self.mixable_as.extend(roles);
        self
    }

    pub fn can_be(&self, role: MixRole) -> bool {
        self.mixable_as.contains(&role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::KeyMode;

    fn sample_key() -> Key {
        Key::new(8, KeyMode::Minor).unwrap()
    }

    fn sample_cue() -> Cue {
        Cue::new(
            TrackId::new(),
            32.0,
            CueType::Drop,
            128.0,
            sample_key(),
            0.8,
            32,
        )
        .unwrap()
    }

    #[test]
    fn construction_succeeds() {
        let cue = sample_cue();
        assert_eq!(cue.position_beats, 32.0);
        assert_eq!(cue.cue_type, CueType::Drop);
        // 既定の compatible_energy は ±0.2 を [0,1] にクランプ
        assert!((cue.compatible_energy.start - 0.6).abs() < 1e-6);
        assert!((cue.compatible_energy.end - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rejects_negative_position() {
        let err = Cue::new(
            TrackId::new(),
            -1.0,
            CueType::HotCue,
            128.0,
            sample_key(),
            0.5,
            16,
        );
        assert!(err.is_err());
    }

    #[test]
    fn rejects_bad_energy() {
        let err = Cue::new(
            TrackId::new(),
            0.0,
            CueType::HotCue,
            128.0,
            sample_key(),
            1.5,
            16,
        );
        assert!(err.is_err());
    }

    #[test]
    fn mix_roles_dedup() {
        let cue = sample_cue().with_mix_roles([MixRole::Entry, MixRole::Entry, MixRole::Exit]);
        assert_eq!(cue.mixable_as.len(), 2);
        assert!(cue.can_be(MixRole::Entry));
        assert!(cue.can_be(MixRole::Exit));
    }

    #[test]
    fn section_requires_valid_range() {
        let cue = sample_cue();
        assert!(cue.clone().with_section(0.0..32.0).is_ok());
        assert!(cue.clone().with_section(32.0..32.0).is_err());
        assert!(cue.with_section(32.0..16.0).is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let cue = sample_cue().with_mix_roles([MixRole::Entry]);
        let json = serde_json::to_string(&cue).unwrap();
        let back: Cue = serde_json::from_str(&json).unwrap();
        assert_eq!(cue, back);
    }
}
