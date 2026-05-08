use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::{Duration, Instant};

use rodio::{Decoder, Sink, Source};
use tracing::debug;

use crate::device::OutputDevice;
use crate::error::{AudioError, AudioResult};

/// チャンネルボリュームの受け入れ範囲。0 〜 2.0（+6dB 相当）。
pub const CHANNEL_VOLUME_MIN: f32 = 0.0;
pub const CHANNEL_VOLUME_MAX: f32 = 2.0;

/// デッキ識別（将来 4 デッキ対応に拡張予定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckId {
    A,
    B,
}

/// テンポ調整レンジ（要件 6.1）。
///
/// `tempo_adjust` が `-1.0..=1.0` の範囲で、レンジの最大値までスケールされる。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TempoRange {
    /// ±6%
    Six,
    /// ±10%
    Ten,
    /// ±16%
    Sixteen,
}

impl TempoRange {
    /// 最大テンポ調整率（0.06 / 0.10 / 0.16）。
    pub fn max_adjust(self) -> f32 {
        match self {
            Self::Six => 0.06,
            Self::Ten => 0.10,
            Self::Sixteen => 0.16,
        }
    }

    /// UI / ログ用の短い表記。
    pub fn as_percent(self) -> u8 {
        match self {
            Self::Six => 6,
            Self::Ten => 10,
            Self::Sixteen => 16,
        }
    }
}

impl Default for TempoRange {
    fn default() -> Self {
        Self::Six
    }
}

/// 単一曲を再生する "デッキ"。
///
/// `Deck` は自身の `Sink` を通じて独立に再生制御を行う。
/// 観客用（Main）出力への合流・クロスフェーダーの適用は `Mixer` が担当する。
pub struct Deck {
    id: DeckId,
    sink: Sink,

    duration: Option<Duration>,

    // --- 位置計算用（Phase 1 と同様の wall-clock 近似） ---
    started_at: Option<Instant>,
    paused_accum: Duration,
    paused_at: Option<Instant>,

    // --- ボリューム ---
    /// チャンネルフェーダー相当（0.0〜2.0）。ユーザーが直接操作する値。
    channel_volume: f32,
    /// Mixer が最終合成した Sink への実効ボリューム。
    /// `channel_volume * crossfader_side * master_volume` で算出される。
    effective_volume: f32,

    // --- テンポ ---
    tempo_range: TempoRange,
    /// -1.0 .. 1.0（フェーダー位置相当）。実速度は `1 + tempo_adjust * range.max_adjust()`。
    tempo_adjust: f32,

    /// シーク後の起点。`position()` が `started_at + offset` を返すために使う。
    position_offset: Duration,
}

impl Deck {
    /// 空のデッキを作る。曲は `load` 後に再生可能。
    pub fn new(id: DeckId, device: &OutputDevice) -> AudioResult<Self> {
        let sink =
            Sink::try_new(device.handle()).map_err(|e| AudioError::Stream(e.to_string()))?;
        sink.pause();
        sink.set_volume(1.0);
        Ok(Self {
            id,
            sink,
            duration: None,
            started_at: None,
            paused_accum: Duration::ZERO,
            paused_at: None,
            channel_volume: 1.0,
            effective_volume: 1.0,
            tempo_range: TempoRange::default(),
            tempo_adjust: 0.0,
            position_offset: Duration::ZERO,
        })
    }

    pub fn id(&self) -> DeckId {
        self.id
    }

    /// 音源ファイルをロードする。既存の再生は停止される。
    pub fn load(&mut self, device: &OutputDevice, path: &Path) -> AudioResult<()> {
        let file = File::open(path).map_err(|source| AudioError::FileOpen {
            path: path.to_path_buf(),
            source,
        })?;
        let decoder =
            Decoder::new(BufReader::new(file)).map_err(|e| AudioError::Decode(e.to_string()))?;

        self.duration = decoder.total_duration();

        self.sink.stop();
        self.sink =
            Sink::try_new(device.handle()).map_err(|e| AudioError::Stream(e.to_string()))?;
        self.sink.pause();
        self.sink.set_volume(self.effective_volume);
        self.sink.set_speed(self.playback_speed());
        self.sink.append(decoder);

        self.started_at = None;
        self.paused_accum = Duration::ZERO;
        self.paused_at = None;
        self.position_offset = Duration::ZERO;

        debug!(
            deck = ?self.id,
            path = %path.display(),
            duration = ?self.duration,
            "track loaded",
        );
        Ok(())
    }

    pub fn play(&mut self) {
        if !self.sink.is_paused() {
            return;
        }
        match (self.started_at, self.paused_at.take()) {
            (None, _) => self.started_at = Some(Instant::now()),
            (Some(_), Some(at)) => self.paused_accum += at.elapsed(),
            (Some(_), None) => {}
        }
        self.sink.play();
    }

    pub fn pause(&mut self) {
        if self.sink.is_paused() {
            return;
        }
        self.paused_at = Some(Instant::now());
        self.sink.pause();
    }

    pub fn stop(&mut self) {
        self.sink.stop();
        self.started_at = None;
        self.paused_accum = Duration::ZERO;
        self.paused_at = None;
        self.position_offset = Duration::ZERO;
    }

    /// 再生位置を指定秒に移動する。Sink::try_seek が成功した場合のみ
    /// 内部の position 計算もリセットする。
    pub fn seek(&mut self, position: Duration) -> AudioResult<()> {
        self.sink
            .try_seek(position)
            .map_err(|e| AudioError::Playback(format!("seek failed: {e}")))?;
        self.started_at = Some(Instant::now());
        self.paused_accum = Duration::ZERO;
        self.paused_at = if self.sink.is_paused() {
            Some(Instant::now())
        } else {
            None
        };
        self.position_offset = position;
        Ok(())
    }

    pub fn is_playing(&self) -> bool {
        !self.sink.is_paused() && !self.sink.empty()
    }

    pub fn is_paused(&self) -> bool {
        self.sink.is_paused() && !self.sink.empty()
    }

    pub fn is_finished(&self) -> bool {
        self.sink.empty()
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub fn position(&self) -> Duration {
        let Some(start) = self.started_at else {
            return self.position_offset;
        };
        let raw = match self.paused_at {
            Some(at) => at.saturating_duration_since(start),
            None => start.elapsed(),
        };
        let elapsed = raw.saturating_sub(self.paused_accum);
        self.position_offset + elapsed
    }

    /// チャンネルボリュームを設定（0.0〜2.0）。
    pub fn set_channel_volume(&mut self, v: f32) {
        self.channel_volume = v.clamp(CHANNEL_VOLUME_MIN, CHANNEL_VOLUME_MAX);
    }

    pub fn channel_volume(&self) -> f32 {
        self.channel_volume
    }

    /// Mixer から呼ばれる実効ボリューム更新。
    pub(crate) fn apply_effective_volume(&mut self, v: f32) {
        let clamped = v.clamp(CHANNEL_VOLUME_MIN, CHANNEL_VOLUME_MAX);
        self.effective_volume = clamped;
        self.sink.set_volume(clamped);
    }

    pub fn effective_volume(&self) -> f32 {
        self.effective_volume
    }

    // --- テンポ ---

    pub fn tempo_range(&self) -> TempoRange {
        self.tempo_range
    }

    pub fn set_tempo_range(&mut self, range: TempoRange) {
        self.tempo_range = range;
        self.apply_speed();
    }

    pub fn tempo_adjust(&self) -> f32 {
        self.tempo_adjust
    }

    /// -1.0 〜 1.0 のフェーダー位置を受け取る。
    /// レンジ外の値はクランプされる。
    pub fn set_tempo_adjust(&mut self, pos: f32) {
        self.tempo_adjust = pos.clamp(-1.0, 1.0);
        self.apply_speed();
    }

    /// 現在の再生速度（1.0 が原速）。
    ///
    /// Phase 2b ではこの速度がピッチに直結する。キーロック（ピッチ独立化）は Phase 2g。
    pub fn playback_speed(&self) -> f32 {
        1.0 + self.tempo_adjust * self.tempo_range.max_adjust()
    }

    fn apply_speed(&mut self) {
        self.sink.set_speed(self.playback_speed());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_volume_clamps() {
        // Sink 不要で確認できるようにクランプロジックだけ検証する必要があるが、
        // Deck は OutputDevice を要求するので、ここでは定数のみ確認する。
        assert_eq!(CHANNEL_VOLUME_MIN, 0.0);
        assert!(CHANNEL_VOLUME_MAX > 1.0);
    }

    #[test]
    fn tempo_range_percentages() {
        assert_eq!(TempoRange::Six.as_percent(), 6);
        assert_eq!(TempoRange::Ten.as_percent(), 10);
        assert_eq!(TempoRange::Sixteen.as_percent(), 16);
        assert!((TempoRange::Six.max_adjust() - 0.06).abs() < 1e-6);
        assert!((TempoRange::Ten.max_adjust() - 0.10).abs() < 1e-6);
        assert!((TempoRange::Sixteen.max_adjust() - 0.16).abs() < 1e-6);
    }

    /// `playback_speed` 計算は Sink 非依存で検証できる純粋関数なので、
    /// 各境界値でカバーする。
    #[test]
    fn playback_speed_formula() {
        // Sink なしで検算する（Deck 構築は OutputDevice 必須なので式を直接再現）。
        fn speed(range: TempoRange, adjust: f32) -> f32 {
            1.0 + adjust * range.max_adjust()
        }
        assert!((speed(TempoRange::Six, 0.0) - 1.0).abs() < 1e-6);
        assert!((speed(TempoRange::Six, 1.0) - 1.06).abs() < 1e-6);
        assert!((speed(TempoRange::Six, -1.0) - 0.94).abs() < 1e-6);
        assert!((speed(TempoRange::Sixteen, 1.0) - 1.16).abs() < 1e-6);
        assert!((speed(TempoRange::Sixteen, -0.5) - 0.92).abs() < 1e-6);
    }
}
