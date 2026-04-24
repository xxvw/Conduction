use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::{Duration, Instant};

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use tracing::{debug, info};

use crate::error::{AudioError, AudioResult};

/// ゲイン値の受け入れ範囲。0 〜 2.0（+6dB 相当）まで許容する。
const GAIN_MIN: f32 = 0.0;
const GAIN_MAX: f32 = 2.0;

/// 単一曲を再生する最小プレイヤー。
///
/// Phase 1 の目的は「音が出る」ことの確認と、後続フェーズの骨格提供。
/// 2 デッキ並行再生、cpal 直結、Conductor 層からの制御は後続フェーズで拡張する。
pub struct Player {
    /// OutputStream は drop されるとデバイスが閉じるので保持する。
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sink: Sink,

    /// 現在ロード中のソースが報告した再生時間長（不明なら None）。
    duration: Option<Duration>,

    // --- position 計算用 ---
    started_at: Option<Instant>,
    paused_accum: Duration,
    paused_at: Option<Instant>,
}

impl Player {
    /// デフォルト出力デバイスでプレイヤーを開く。
    pub fn new() -> AudioResult<Self> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|e| AudioError::Stream(e.to_string()))?;
        let sink = Sink::try_new(&handle).map_err(|e| AudioError::Stream(e.to_string()))?;
        sink.pause();

        info!("audio player opened on default output device");

        Ok(Self {
            _stream: stream,
            handle,
            sink,
            duration: None,
            started_at: None,
            paused_accum: Duration::ZERO,
            paused_at: None,
        })
    }

    /// 音源ファイルをロードする。既存の再生は停止される。
    ///
    /// 対応フォーマットは rodio の feature で有効化している Symphonia デコーダ
    /// （MP3, WAV, FLAC, Vorbis, AAC）に準じる。
    pub fn load(&mut self, path: &Path) -> AudioResult<()> {
        let file = File::open(path).map_err(|source| AudioError::FileOpen {
            path: path.to_path_buf(),
            source,
        })?;
        let decoder =
            Decoder::new(BufReader::new(file)).map_err(|e| AudioError::Decode(e.to_string()))?;

        self.duration = decoder.total_duration();

        // 既存 Sink は停止して作り直す（Sink には clear がないため）。
        self.sink.stop();
        self.sink =
            Sink::try_new(&self.handle).map_err(|e| AudioError::Stream(e.to_string()))?;
        self.sink.pause();
        self.sink.append(decoder);

        self.started_at = None;
        self.paused_accum = Duration::ZERO;
        self.paused_at = None;

        debug!(
            "track loaded: path={} duration={:?}",
            path.display(),
            self.duration
        );
        Ok(())
    }

    /// 再生開始または一時停止からの再開。
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

    /// 一時停止。
    pub fn pause(&mut self) {
        if self.sink.is_paused() {
            return;
        }
        self.paused_at = Some(Instant::now());
        self.sink.pause();
    }

    /// 再生停止（キューを破棄）。
    pub fn stop(&mut self) {
        self.sink.stop();
        self.started_at = None;
        self.paused_accum = Duration::ZERO;
        self.paused_at = None;
    }

    /// ゲイン（音量倍率）を設定する。`[0.0, 2.0]` にクランプ。
    pub fn set_gain(&self, gain: f32) {
        let clamped = gain.clamp(GAIN_MIN, GAIN_MAX);
        self.sink.set_volume(clamped);
    }

    pub fn gain(&self) -> f32 {
        self.sink.volume()
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

    /// 再生開始からの経過時間を返す（pause 区間は除外）。
    ///
    /// Phase 1 では wall-clock ベースの近似。サンプル精度の位置取得は
    /// Phase 2 以降で cpal 直結に切り替える際に再実装する。
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gain clamp は Sink に依存せず検証できないので、定数のカバレッジだけ確認。
    #[test]
    fn gain_bounds_defined() {
        assert!(GAIN_MIN < GAIN_MAX);
        assert_eq!(GAIN_MIN, 0.0);
    }

    /// 実デバイスに依存するため CI では ignore 指定。ローカル手動実行用。
    #[test]
    #[ignore = "requires audio output device"]
    fn opens_default_device() {
        let _player = Player::new().expect("should open default device");
    }
}
