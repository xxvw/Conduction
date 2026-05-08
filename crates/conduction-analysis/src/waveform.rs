//! 3 バンド (low / mid / high) の RMS 振幅で構成される波形プレビュー生成。
//!
//! rekordbox 風の「全体波形」用途。ビン数 N（既定 1024）で固定し、
//! 楽曲長によらず常に同じ解像度のプレビューを得る。
//!
//! 帯域分離は 2-pole Butterworth IIR で実装：
//! - low  = 入力 → LPF (250 Hz)
//! - high = 入力 → HPF (4 kHz)
//! - mid  = 入力 - low - high   （残差）

use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Type, Q_BUTTERWORTH_F32};
use serde::{Deserialize, Serialize};

use crate::decode::DecodedAudio;

pub const DEFAULT_WAVEFORM_BINS: usize = 1024;

/// バンド分割閾値。一般的な DJ プレビューに合わせた値。
const LOW_CUTOFF_HZ: f32 = 250.0;
const HIGH_CUTOFF_HZ: f32 = 4000.0;

/// ビン化された 3 バンド振幅。各 Vec は `sample_count` 要素、値は 0..=1（最大値で正規化済）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveformPreview {
    pub sample_count: u32,
    pub low: Vec<f32>,
    pub mid: Vec<f32>,
    pub high: Vec<f32>,
}

impl WaveformPreview {
    pub fn empty(bins: usize) -> Self {
        Self {
            sample_count: bins as u32,
            low: vec![0.0; bins],
            mid: vec![0.0; bins],
            high: vec![0.0; bins],
        }
    }
}

/// PCM をビン化し、3 バンド RMS の波形プレビューを生成する。
pub fn generate_waveform(audio: &DecodedAudio, bins: usize) -> WaveformPreview {
    let bins = bins.max(1);
    let total = audio.samples.len();

    if total == 0 || audio.sample_rate == 0 {
        return WaveformPreview::empty(bins);
    }

    let sr = audio.sample_rate as f32;

    // フィルタ初期化（コンパイル時定数 cutoff なので unwrap は実質安全）。
    let lp_coef = Coefficients::<f32>::from_params(
        Type::LowPass,
        sr.hz(),
        LOW_CUTOFF_HZ.hz(),
        Q_BUTTERWORTH_F32,
    )
    .expect("static cutoff is valid");
    let mut lp = DirectForm1::<f32>::new(lp_coef);

    let hp_coef = Coefficients::<f32>::from_params(
        Type::HighPass,
        sr.hz(),
        HIGH_CUTOFF_HZ.hz(),
        Q_BUTTERWORTH_F32,
    )
    .expect("static cutoff is valid");
    let mut hp = DirectForm1::<f32>::new(hp_coef);

    let samples_per_bin = total.div_ceil(bins);

    // ビン毎の (sum_sq, count)。f64 で蓄積して桁落ちを防ぐ。
    let mut low_acc = vec![(0.0_f64, 0_u64); bins];
    let mut mid_acc = vec![(0.0_f64, 0_u64); bins];
    let mut high_acc = vec![(0.0_f64, 0_u64); bins];

    for (i, &s) in audio.samples.iter().enumerate() {
        let l = lp.run(s);
        let h = hp.run(s);
        let m = s - l - h;

        let bin = (i / samples_per_bin).min(bins - 1);
        low_acc[bin].0 += (l as f64) * (l as f64);
        low_acc[bin].1 += 1;
        mid_acc[bin].0 += (m as f64) * (m as f64);
        mid_acc[bin].1 += 1;
        high_acc[bin].0 += (h as f64) * (h as f64);
        high_acc[bin].1 += 1;
    }

    let mut low = Vec::with_capacity(bins);
    let mut mid = Vec::with_capacity(bins);
    let mut high = Vec::with_capacity(bins);
    for i in 0..bins {
        low.push(rms(low_acc[i]));
        mid.push(rms(mid_acc[i]));
        high.push(rms(high_acc[i]));
    }

    // 全バンド合算の最大ピークで正規化。バンド間の相対関係を維持する。
    let max = low
        .iter()
        .chain(mid.iter())
        .chain(high.iter())
        .copied()
        .fold(0.0_f32, f32::max);
    if max > 1e-9 {
        let inv = 1.0 / max;
        for v in low.iter_mut() {
            *v *= inv;
        }
        for v in mid.iter_mut() {
            *v *= inv;
        }
        for v in high.iter_mut() {
            *v *= inv;
        }
    }

    WaveformPreview {
        sample_count: bins as u32,
        low,
        mid,
        high,
    }
}

fn rms((sum_sq, count): (f64, u64)) -> f32 {
    if count == 0 {
        0.0
    } else {
        (sum_sq / count as f64).sqrt() as f32
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;

    fn sine(freq_hz: f32, secs: f32, sr: u32) -> DecodedAudio {
        let n = (sr as f32 * secs) as usize;
        let mut s = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / sr as f32;
            s.push((2.0 * PI * freq_hz * t).sin());
        }
        DecodedAudio {
            samples: s,
            sample_rate: sr,
        }
    }

    fn mean(v: &[f32]) -> f32 {
        if v.is_empty() {
            0.0
        } else {
            v.iter().sum::<f32>() / v.len() as f32
        }
    }

    #[test]
    fn empty_input_yields_zeros() {
        let audio = DecodedAudio {
            samples: vec![],
            sample_rate: 44100,
        };
        let wf = generate_waveform(&audio, 64);
        assert_eq!(wf.sample_count, 64);
        assert!(wf.low.iter().all(|v| *v == 0.0));
        assert!(wf.mid.iter().all(|v| *v == 0.0));
        assert!(wf.high.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn low_freq_dominates_low_band() {
        let audio = sine(60.0, 1.0, 44100);
        let wf = generate_waveform(&audio, 32);
        // 過渡応答の最初のビンを除外して中盤以降だけ見る。
        let lo = mean(&wf.low[8..]);
        let hi = mean(&wf.high[8..]);
        assert!(
            lo > hi * 5.0,
            "low band ({lo:.4}) should dominate high band ({hi:.4}) for 60Hz sine"
        );
    }

    #[test]
    fn high_freq_dominates_high_band() {
        let audio = sine(8000.0, 1.0, 44100);
        let wf = generate_waveform(&audio, 32);
        let lo = mean(&wf.low[8..]);
        let hi = mean(&wf.high[8..]);
        assert!(
            hi > lo * 5.0,
            "high band ({hi:.4}) should dominate low band ({lo:.4}) for 8kHz sine"
        );
    }

    #[test]
    fn mid_freq_dominates_mid_band() {
        let audio = sine(1000.0, 1.0, 44100);
        let wf = generate_waveform(&audio, 32);
        let lo = mean(&wf.low[8..]);
        let mi = mean(&wf.mid[8..]);
        let hi = mean(&wf.high[8..]);
        assert!(
            mi > lo && mi > hi,
            "mid band ({mi:.4}) should dominate (low={lo:.4}, high={hi:.4}) for 1kHz sine"
        );
    }

    #[test]
    fn output_normalized_to_unit() {
        let audio = sine(1000.0, 0.5, 44100);
        let wf = generate_waveform(&audio, 64);
        let max = wf
            .low
            .iter()
            .chain(wf.mid.iter())
            .chain(wf.high.iter())
            .copied()
            .fold(0.0_f32, f32::max);
        assert!(max > 0.0 && max <= 1.0001);
    }
}
