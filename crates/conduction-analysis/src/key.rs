//! Krumhansl-Schmuckler によるキー検出。
//!
//! 1. PCM をモノラル化して STFT (Hann 窓) でショートタイム magnitude spectrum を出す。
//! 2. 各フレームを **chroma vector (12 次元 / pitch class)** に集約する。
//! 3. 全フレームの平均クロマと、24 種のキープロファイル (12 major + 12 minor) を Pearson 相関で比較。
//! 4. 最高相関のプロファイルを楽曲のキーとして返す。
//!
//! 参考: Krumhansl, C. (1990). *Cognitive Foundations of Musical Pitch*.

use std::sync::Arc;

use conduction_core::{Key, KeyMode};
use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use serde::{Deserialize, Serialize};

use crate::decode::DecodedAudio;

/// Krumhansl-Schmuckler の major プロファイル (C major、C 始まり)。
const MAJOR_PROFILE: [f32; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
/// Krumhansl-Schmuckler の minor プロファイル (A minor → C 始まりに回転、すなわち C minor 起点)。
const MINOR_PROFILE: [f32; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// FFT サイズ (約 93ms @ 44.1kHz)。半音解像度を確保するため 4096。
const FFT_SIZE: usize = 4096;
/// hop 間隔。50% オーバーラップ。
const HOP_SIZE: usize = 2048;

/// pitch class (0=C, 1=C#, ..., 11=B) → Camelot 番号。
const MAJOR_TO_CAMELOT: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];
const MINOR_TO_CAMELOT: [u8; 12] = [5, 12, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEstimate {
    pub key: Key,
    /// Pearson 相関 (-1..=1)。1 に近いほど自信が高い。
    pub correlation: f32,
    /// 1 位と 2 位の差 (相関スコアの margin)。曖昧さの指標。
    pub margin: f32,
}

/// 楽曲全体のキーを 1 つだけ推定する。
pub fn estimate_key(audio: &DecodedAudio) -> Option<KeyEstimate> {
    if audio.samples.len() < FFT_SIZE * 4 || audio.sample_rate == 0 {
        return None;
    }

    let chroma = compute_average_chroma(&audio.samples, audio.sample_rate);
    if chroma.iter().all(|x| *x == 0.0) {
        return None;
    }

    // 24 候補とのスコアを計算し、最大と次点を取る。
    let mut best = (0.0f32, 0u8, KeyMode::Major);
    let mut second = f32::NEG_INFINITY;
    for pc in 0..12u8 {
        for mode in [KeyMode::Major, KeyMode::Minor] {
            let profile = rotate_profile(mode, pc as usize);
            let r = pearson(&chroma, &profile);
            if r > best.0 {
                second = best.0;
                best = (r, pc, mode);
            } else if r > second {
                second = r;
            }
        }
    }

    let camelot = match best.2 {
        KeyMode::Major => MAJOR_TO_CAMELOT[best.1 as usize],
        KeyMode::Minor => MINOR_TO_CAMELOT[best.1 as usize],
    };
    let key = Key::new(camelot, best.2).ok()?;
    let margin = (best.0 - second).max(0.0);
    Some(KeyEstimate {
        key,
        correlation: best.0,
        margin,
    })
}

fn rotate_profile(mode: KeyMode, root_pc: usize) -> [f32; 12] {
    let base = match mode {
        KeyMode::Major => &MAJOR_PROFILE,
        KeyMode::Minor => &MINOR_PROFILE,
    };
    let mut out = [0.0f32; 12];
    for i in 0..12 {
        out[(i + root_pc) % 12] = base[i];
    }
    out
}

fn pearson(a: &[f32; 12], b: &[f32; 12]) -> f32 {
    let mean_a: f32 = a.iter().sum::<f32>() / 12.0;
    let mean_b: f32 = b.iter().sum::<f32>() / 12.0;
    let mut num = 0.0f32;
    let mut da = 0.0f32;
    let mut db = 0.0f32;
    for i in 0..12 {
        let xa = a[i] - mean_a;
        let xb = b[i] - mean_b;
        num += xa * xb;
        da += xa * xa;
        db += xb * xb;
    }
    let den = (da * db).sqrt();
    if den == 0.0 {
        0.0
    } else {
        num / den
    }
}

fn compute_average_chroma(samples: &[f32], sample_rate: u32) -> [f32; 12] {
    let mut planner = FftPlanner::<f32>::new();
    let fft: Arc<dyn Fft<f32>> = planner.plan_fft_forward(FFT_SIZE);

    let window = hann_window(FFT_SIZE);
    let mut buf = vec![Complex32::default(); FFT_SIZE];
    let mut chroma_sum = [0.0f32; 12];
    let mut frame_count = 0u32;

    let bin_to_pitch = precompute_bin_to_pitch(sample_rate);

    let mut i = 0;
    while i + FFT_SIZE <= samples.len() {
        // 窓掛け & 複素数配列にコピー
        for k in 0..FFT_SIZE {
            buf[k] = Complex32::new(samples[i + k] * window[k], 0.0);
        }
        fft.process(&mut buf);
        // 半分のビン (Nyquist まで) のマグニチュードを pitch class にビニング。
        let mut frame_chroma = [0.0f32; 12];
        for k in 1..FFT_SIZE / 2 {
            if let Some(pc) = bin_to_pitch[k] {
                let mag = buf[k].norm();
                frame_chroma[pc as usize] += mag;
            }
        }
        // フレームごとに L2 正規化してから累積 (大音量フレームに偏らないように)。
        let norm: f32 = frame_chroma.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for j in 0..12 {
                chroma_sum[j] += frame_chroma[j] / norm;
            }
            frame_count += 1;
        }
        i += HOP_SIZE;
    }

    if frame_count == 0 {
        return [0.0; 12];
    }
    let mut out = [0.0f32; 12];
    for j in 0..12 {
        out[j] = chroma_sum[j] / frame_count as f32;
    }
    out
}

/// 各 FFT ビンを最も近い pitch class (0..=11) にマップ。
/// MIDI ノートが [27, 96] の範囲に入るビンのみ採用 (ベース過多と上倍音雑音を除外)。
fn precompute_bin_to_pitch(sample_rate: u32) -> Vec<Option<u8>> {
    let bin_hz = sample_rate as f32 / FFT_SIZE as f32;
    let mut out = Vec::with_capacity(FFT_SIZE / 2);
    out.push(None); // DC
    for k in 1..FFT_SIZE / 2 {
        let f = k as f32 * bin_hz;
        if f < 27.5 || f > 4186.0 {
            // A0..C8 の範囲外は捨てる。
            out.push(None);
            continue;
        }
        let midi = 12.0 * (f / 440.0).log2() + 69.0;
        let pc = ((midi.round() as i32).rem_euclid(12)) as u8;
        out.push(Some(pc));
    }
    out
}

fn hann_window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let x = (i as f32) / (n as f32 - 1.0);
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * x).cos())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_tone(freq: f32, sample_rate: u32, secs: f32) -> Vec<f32> {
        let n = (sample_rate as f32 * secs) as usize;
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32).sin() * 0.5)
            .collect()
    }

    #[test]
    fn detects_a_major_chord() {
        // A major triad (A4 + C#5 + E5) — Camelot 11B
        let sr = 44100u32;
        let mut samples = synth_tone(440.0, sr, 4.0);
        let c_sharp = synth_tone(554.37, sr, 4.0);
        let e = synth_tone(659.25, sr, 4.0);
        for (i, s) in samples.iter_mut().enumerate() {
            *s = (*s + c_sharp[i] + e[i]) / 3.0;
        }
        let audio = DecodedAudio {
            samples,
            sample_rate: sr,
        };
        let est = estimate_key(&audio).expect("must estimate");
        assert_eq!(est.key.to_camelot(), "11B", "expected A major (11B)");
    }

    #[test]
    fn detects_a_minor_chord() {
        // A minor triad (A4 + C5 + E5) — Camelot 8A
        let sr = 44100u32;
        let mut samples = synth_tone(440.0, sr, 4.0);
        let c = synth_tone(523.25, sr, 4.0);
        let e = synth_tone(659.25, sr, 4.0);
        for (i, s) in samples.iter_mut().enumerate() {
            *s = (*s + c[i] + e[i]) / 3.0;
        }
        let audio = DecodedAudio {
            samples,
            sample_rate: sr,
        };
        let est = estimate_key(&audio).expect("must estimate");
        assert_eq!(est.key.to_camelot(), "8A", "expected A minor (8A)");
    }
}
