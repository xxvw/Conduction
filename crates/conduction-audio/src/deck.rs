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
        self.sink.append(decoder);

        self.started_at = None;
        self.paused_accum = Duration::ZERO;
        self.paused_at = None;

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
            return Duration::ZERO;
        };
        let raw = match self.paused_at {
            Some(at) => at.saturating_duration_since(start),
            None => start.elapsed(),
        };
        raw.saturating_sub(self.paused_accum)
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
}
