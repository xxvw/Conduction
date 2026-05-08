//! 自己相関ベースの BPM / ビートグリッド推定。
//!
//! 高精度な実装は Phase 3b 後半で aubio-rs に置き換える前提。
//! ここでは外部依存なしで実用的な精度（±数 BPM）を狙う。

use conduction_core::Beat;
use serde::{Deserialize, Serialize};

use crate::decode::DecodedAudio;

/// 受け入れる BPM レンジ。テクノ〜D&B までを概ねカバー。
const BPM_MIN: f64 = 70.0;
const BPM_MAX: f64 = 180.0;

/// オンセット強度関数の hop / frame サイズ（サンプル）。
const HOP_SIZE: usize = 512;
const FRAME_SIZE: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatgridEstimate {
    pub bpm: f32,
    pub first_beat_sec: f64,
    pub beat_interval_sec: f64,
    /// 自己相関ピークの正規化スコア（0..=1）。
    pub confidence: f32,
}

impl BeatgridEstimate {
    /// 推定値から `Beat` のシーケンスを生成する（4 拍ごとに `is_downbeat` を立てる）。
    pub fn beats(&self, total_duration_sec: f64) -> Vec<Beat> {
        let mut out = Vec::new();
        if self.beat_interval_sec <= 0.0 {
            return out;
        }
        // first_beat_sec が大きすぎる場合は前にずらして 0 起点に近づける
        let mut t = self.first_beat_sec;
        while t > self.beat_interval_sec {
            t -= self.beat_interval_sec;
        }
        let mut idx = 0usize;
        while t < total_duration_sec {
            out.push(Beat::new(t, idx % 4 == 0));
            t += self.beat_interval_sec;
            idx += 1;
        }
        out
    }
}

/// 楽曲全体の BPM を 1 つだけ推定し、最初のビート位置とともに返す。
pub fn estimate_beatgrid(audio: &DecodedAudio) -> Option<BeatgridEstimate> {
    if audio.samples.len() < FRAME_SIZE * 8 || audio.sample_rate == 0 {
        return None;
    }

    let frame_rate = audio.sample_rate as f64 / HOP_SIZE as f64;

    // 1. オンセット強度関数（RMS の正方向差分）
    let onset = onset_strength(&audio.samples);
    if onset.is_empty() {
        return None;
    }

    // 2. 自己相関で最頻ラグを探す
    let lag_min = ((60.0 / BPM_MAX) * frame_rate).max(2.0) as usize;
    let lag_max = ((60.0 / BPM_MIN) * frame_rate) as usize;
    let lag_max = lag_max.min(onset.len().saturating_sub(2));
    if lag_max <= lag_min {
        return None;
    }

    let total_energy: f64 = onset.iter().map(|x| (*x as f64).powi(2)).sum();
    if total_energy <= 0.0 {
        return None;
    }

    let mut best_corr = 0.0_f64;
    let mut best_lag = lag_min;
    for lag in lag_min..=lag_max {
        let mut corr = 0.0_f64;
        let limit = onset.len() - lag;
        for i in 0..limit {
            corr += onset[i] as f64 * onset[i + lag] as f64;
        }
        if corr > best_corr {
            best_corr = corr;
            best_lag = lag;
        }
    }

    let beat_interval_sec = best_lag as f64 / frame_rate;
    let bpm = 60.0 / beat_interval_sec;

    // 3. 最初のビート位置: 最初の 1 ビート区間内で最大のオンセット
    let search_end = best_lag.min(onset.len());
    let first_beat_frame = onset[..search_end]
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let first_beat_sec = first_beat_frame as f64 / frame_rate;

    let confidence = (best_corr / total_energy).clamp(0.0, 1.0) as f32;

    Some(BeatgridEstimate {
        bpm: bpm as f32,
        first_beat_sec,
        beat_interval_sec,
        confidence,
    })
}

fn onset_strength(samples: &[f32]) -> Vec<f32> {
    let n = samples.len() / HOP_SIZE;
    if n == 0 {
        return Vec::new();
    }

    // 1. 短時間 RMS
    let mut rms_series = Vec::with_capacity(n);
    for i in 0..n {
        let start = i * HOP_SIZE;
        let end = (start + FRAME_SIZE).min(samples.len());
        let len = end - start;
        if len == 0 {
            rms_series.push(0.0);
            continue;
        }
        let mut sum_sq = 0.0_f64;
        for &s in &samples[start..end] {
            sum_sq += (s as f64) * (s as f64);
        }
        rms_series.push((sum_sq / len as f64).sqrt() as f32);
    }

    // 2. 正方向差分（spectral flux の time-domain 簡易版）
    let mut onset = Vec::with_capacity(rms_series.len());
    onset.push(0.0);
    for i in 1..rms_series.len() {
        onset.push((rms_series[i] - rms_series[i - 1]).max(0.0));
    }
    onset
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;

    /// 一定 BPM の "クリック音"（規則的なインパルス）を生成して BPM を回復できるか検証。
    fn click_track(bpm: f64, secs: f32, sr: u32) -> DecodedAudio {
        let total = (sr as f32 * secs) as usize;
        let mut s = vec![0.0_f32; total];
        let interval_samples = ((60.0 / bpm) * sr as f64) as usize;
        let click_freq = 1000.0_f32; // 1 kHz サイン
        let click_len = (sr / 80) as usize; // ~12 ms

        let mut beat = 0usize;
        while beat < total {
            for i in 0..click_len.min(total - beat) {
                let t = i as f32 / sr as f32;
                let env = 1.0 - (i as f32 / click_len as f32);
                s[beat + i] += (2.0 * PI * click_freq * t).sin() * env * 0.7;
            }
            beat += interval_samples;
        }
        DecodedAudio {
            samples: s,
            sample_rate: sr,
        }
    }

    #[test]
    fn recovers_120_bpm() {
        let audio = click_track(120.0, 8.0, 44100);
        let est = estimate_beatgrid(&audio).expect("should detect");
        assert!(
            (est.bpm - 120.0).abs() < 4.0,
            "expected ~120 BPM, got {}",
            est.bpm
        );
    }

    #[test]
    fn recovers_140_bpm() {
        let audio = click_track(140.0, 8.0, 44100);
        let est = estimate_beatgrid(&audio).expect("should detect");
        assert!(
            (est.bpm - 140.0).abs() < 4.0,
            "expected ~140 BPM, got {}",
            est.bpm
        );
    }

    #[test]
    fn beats_within_duration() {
        let est = BeatgridEstimate {
            bpm: 128.0,
            first_beat_sec: 0.1,
            beat_interval_sec: 60.0 / 128.0,
            confidence: 0.5,
        };
        let beats = est.beats(10.0);
        assert!(beats.len() > 15);
        assert!(beats.iter().all(|b| b.position_sec < 10.0));
        // 4 拍ごとに downbeat
        let downs = beats.iter().filter(|b| b.is_downbeat).count();
        assert!(downs >= beats.len() / 4 - 1);
    }

    #[test]
    fn empty_audio_returns_none() {
        let audio = DecodedAudio {
            samples: vec![],
            sample_rate: 44100,
        };
        assert!(estimate_beatgrid(&audio).is_none());
    }
}
