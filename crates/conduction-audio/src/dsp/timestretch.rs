//! Pitch-preserving time-stretch + pitch-shift adapter (rodio Source).
//!
//! `SoundTouch` を内部に持ち、入力 `Source<Item=f32>` をブロック単位でリサンプル/
//! ピッチシフトしてから返す。tempo / pitch は `Arc<TimeStretchParams>` 経由で
//! UI スレッドから lock-free に更新される。
//!
//! 設計上の注意:
//! - `SoundTouch` は `Send` だが `!Sync`。audio スレッドのみが触る。
//! - tempo == 1.0 / pitch == 0 のときも内部処理は走る (SoundTouch は適切に
//!   pass-through する)。
//! - try_seek 時は `clear()` で内部バッファを破棄して位相不整合を防ぐ。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::Source;
use soundtouch::SoundTouch;

/// 1 度に SoundTouch に push するフレーム数 (= サンプル数 / チャンネル数)。
/// 大きいと latency が上がるが SoundTouch の内部窓と整合しやすい。
const BLOCK_FRAMES: usize = 1024;

#[derive(Debug)]
pub struct TimeStretchParams {
    /// f32 を AtomicU32 (to_bits / from_bits) で保持。
    tempo_bits: AtomicU32,
    /// 半音単位。整数値で SoundTouch に流す (set_pitch_semitones は i32)。
    pitch_semitones_bits: AtomicU32,
}

impl TimeStretchParams {
    pub fn new() -> Self {
        Self {
            tempo_bits: AtomicU32::new(1.0f32.to_bits()),
            pitch_semitones_bits: AtomicU32::new(0.0f32.to_bits()),
        }
    }

    pub fn new_arc() -> Arc<Self> {
        Arc::new(Self::new())
    }

    pub fn tempo(&self) -> f32 {
        f32::from_bits(self.tempo_bits.load(Ordering::Relaxed))
    }

    pub fn set_tempo(&self, v: f32) {
        // 0.25..=4.0 で clamp。SoundTouch 推奨範囲。
        let clamped = v.clamp(0.25, 4.0);
        self.tempo_bits.store(clamped.to_bits(), Ordering::Relaxed);
    }

    pub fn pitch_semitones(&self) -> f32 {
        f32::from_bits(self.pitch_semitones_bits.load(Ordering::Relaxed))
    }

    pub fn set_pitch_semitones(&self, v: f32) {
        let clamped = v.clamp(-12.0, 12.0);
        self.pitch_semitones_bits
            .store(clamped.to_bits(), Ordering::Relaxed);
    }
}

impl Default for TimeStretchParams {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TimeStretchSource<S: Source<Item = f32>> {
    inner: S,
    params: Arc<TimeStretchParams>,
    touch: SoundTouch,
    last_tempo: f32,
    last_pitch_st: i32,
    sample_rate: u32,
    channels: u16,
    /// `receive_samples` で取り出したインターリーブ済みサンプル。
    out_buffer: VecDeque<f32>,
    /// 入力が EOF に達したか。一度 true になったら再 fill しない。
    inner_eof: bool,
}

impl<S: Source<Item = f32>> TimeStretchSource<S> {
    pub fn new(inner: S, params: Arc<TimeStretchParams>) -> Self {
        let sample_rate = inner.sample_rate();
        let channels = inner.channels();
        let mut touch = SoundTouch::new();
        touch
            .set_sample_rate(sample_rate)
            .set_channels(channels.max(1) as u32)
            .set_tempo(1.0)
            .set_pitch_semitones(0);
        Self {
            inner,
            params,
            touch,
            last_tempo: 1.0,
            last_pitch_st: 0,
            sample_rate,
            channels,
            out_buffer: VecDeque::with_capacity(BLOCK_FRAMES * 2 * channels as usize),
            inner_eof: false,
        }
    }

    /// 入力から 1 ブロック吸い込んで SoundTouch に push し、出力を `out_buffer` に貯める。
    /// 戻り値: 1 サンプル以上吸えなかった場合 `false` (= 完全 EOF + 出力もなし)。
    fn fill_block(&mut self) -> bool {
        // 1. パラメータ更新
        let target_tempo = self.params.tempo();
        let target_pitch = self.params.pitch_semitones().round() as i32;
        if (target_tempo - self.last_tempo).abs() > 1e-3 {
            self.touch.set_tempo(target_tempo as f64);
            self.last_tempo = target_tempo;
        }
        if target_pitch != self.last_pitch_st {
            self.touch.set_pitch_semitones(target_pitch);
            self.last_pitch_st = target_pitch;
        }

        // 2. 入力 BLOCK_FRAMES * channels 分取得
        let chans = self.channels.max(1) as usize;
        let want = BLOCK_FRAMES * chans;
        let mut block: Vec<f32> = Vec::with_capacity(want);
        for _ in 0..want {
            match self.inner.next() {
                Some(s) => block.push(s),
                None => {
                    self.inner_eof = true;
                    break;
                }
            }
        }

        // 3. SoundTouch に push (frame 数で渡す)
        if !block.is_empty() {
            let frames = block.len() / chans;
            self.touch.put_samples(&block, frames);
        }
        if self.inner_eof {
            self.touch.flush();
        }

        // 4. 取り出せるだけ取り出す (frame 単位)
        let mut tmp = vec![0.0f32; BLOCK_FRAMES * chans];
        loop {
            let max_frames = BLOCK_FRAMES;
            let got_frames = self.touch.receive_samples(&mut tmp, max_frames);
            if got_frames == 0 {
                break;
            }
            let got_samples = got_frames * chans;
            self.out_buffer.extend(tmp[..got_samples].iter().copied());
        }

        !self.out_buffer.is_empty() || !self.inner_eof
    }
}

impl<S: Source<Item = f32>> Iterator for TimeStretchSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        loop {
            if let Some(s) = self.out_buffer.pop_front() {
                return Some(s);
            }
            if self.inner_eof {
                return None;
            }
            if !self.fill_block() {
                return None;
            }
        }
    }
}

impl<S: Source<Item = f32>> Source for TimeStretchSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        self.touch.clear();
        self.out_buffer.clear();
        self.inner_eof = false;
        Ok(())
    }
}
