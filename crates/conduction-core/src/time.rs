use serde::{Deserialize, Serialize};

/// テンプレート内の時間単位。
/// 拍数ベースをメインとし、特定イベントは絶対時間も許容する（要件 6.6）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum TimePosition {
    /// 先頭（0 拍目）からの拍数。
    Beats(f64),
    /// 絶対時間（秒）。
    Seconds(f64),
    /// 終了からの拍数（逆算）。
    BeatsFromEnd(f64),
}

impl TimePosition {
    #[inline]
    pub const fn beats(value: f64) -> Self {
        Self::Beats(value)
    }

    #[inline]
    pub const fn seconds(value: f64) -> Self {
        Self::Seconds(value)
    }

    #[inline]
    pub const fn beats_from_end(value: f64) -> Self {
        Self::BeatsFromEnd(value)
    }

    /// BPM と全体の拍数があれば絶対秒に正規化する。
    ///
    /// * `bpm` — 曲全体の代表 BPM。
    /// * `total_beats` — テンプレート全体の拍数。`BeatsFromEnd` 解決に必要。
    pub fn to_seconds(self, bpm: f32, total_beats: f64) -> f64 {
        let beat_duration_secs = 60.0 / bpm as f64;
        match self {
            Self::Seconds(s) => s,
            Self::Beats(b) => b * beat_duration_secs,
            Self::BeatsFromEnd(b) => (total_beats - b).max(0.0) * beat_duration_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beats_to_seconds() {
        // 128 BPM, 4 拍 = 1.875 秒
        let pos = TimePosition::beats(4.0);
        let s = pos.to_seconds(128.0, 128.0);
        assert!((s - 1.875).abs() < 1e-9);
    }

    #[test]
    fn beats_from_end_resolves_against_total() {
        // 全 64 拍、末尾 8 拍手前 = 56 拍目 = 26.25 秒 @ 128 BPM
        let pos = TimePosition::beats_from_end(8.0);
        let s = pos.to_seconds(128.0, 64.0);
        let expected = 56.0 * (60.0 / 128.0);
        assert!((s - expected).abs() < 1e-9);
    }

    #[test]
    fn beats_from_end_saturates_to_zero() {
        // end より大きな逆算指定はゼロで止める
        let pos = TimePosition::beats_from_end(100.0);
        let s = pos.to_seconds(128.0, 16.0);
        assert_eq!(s, 0.0);
    }
}
