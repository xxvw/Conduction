//! Decoder/Source の出力を 3 バンド EQ → Filter → Echo → Reverb に通すアダプタ。
//!
//! チャンネル毎に独立したフィルタ状態を持つ（ステレオ ⇒ 各 2 系統）。
//! パラメータは `DspParams` 経由で UI スレッドから lock-free に書き換えられる。
//! 毎サンプル atomic load は重いので、64 サンプルごとに係数キャッシュを更新する。

use std::sync::Arc;
use std::time::Duration;

use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Type, Q_BUTTERWORTH_F32};
use rodio::Source;

use super::coefficients::{high_shelf, low_shelf, peaking_eq};
use super::echo::EchoEffect;
use super::params::DspParams;
use super::reverb::SchroederReverb;

const PARAM_REFRESH_INTERVAL: u32 = 64;
const EQ_LOW_HZ: f32 = 250.0;
const EQ_MID_HZ: f32 = 1000.0;
const EQ_HIGH_HZ: f32 = 4000.0;
const EQ_Q: f32 = 0.7;

pub struct DjEffectSource<S> {
    inner: S,
    params: Arc<DspParams>,
    sample_rate: u32,
    channels: u16,

    eq_low: Vec<DirectForm1<f32>>,
    eq_mid: Vec<DirectForm1<f32>>,
    eq_high: Vec<DirectForm1<f32>>,
    filter_state: Vec<Option<DirectForm1<f32>>>,
    echo: Vec<EchoEffect>,
    reverb: Vec<SchroederReverb>,

    sample_count: u32,
    current_channel: u16,

    cached_eq_low: f32,
    cached_eq_mid: f32,
    cached_eq_high: f32,
    cached_filter: f32,
    cached_reverb_room: f32,
}

impl<S> DjEffectSource<S>
where
    S: Source<Item = f32>,
{
    pub fn new(inner: S, params: Arc<DspParams>) -> Self {
        let sample_rate = inner.sample_rate();
        let channels = inner.channels().max(1);
        let n = channels as usize;
        let fs = sample_rate as f32;

        let make_filter = |coeffs: Coefficients<f32>| DirectForm1::<f32>::new(coeffs);

        let low_co = low_shelf(fs, EQ_LOW_HZ, EQ_Q, 0.0);
        let mid_co = peaking_eq(fs, EQ_MID_HZ, EQ_Q, 0.0);
        let high_co = high_shelf(fs, EQ_HIGH_HZ, EQ_Q, 0.0);

        let mut s = Self {
            inner,
            params,
            sample_rate,
            channels,

            eq_low: (0..n).map(|_| make_filter(low_co)).collect(),
            eq_mid: (0..n).map(|_| make_filter(mid_co)).collect(),
            eq_high: (0..n).map(|_| make_filter(high_co)).collect(),
            filter_state: vec![None; n],
            echo: (0..n).map(|_| EchoEffect::new(fs)).collect(),
            reverb: (0..n).map(|_| SchroederReverb::new(fs)).collect(),

            sample_count: 0,
            current_channel: 0,

            cached_eq_low: f32::NAN,
            cached_eq_mid: f32::NAN,
            cached_eq_high: f32::NAN,
            cached_filter: f32::NAN,
            cached_reverb_room: f32::NAN,
        };
        s.refresh_coefficients(true);
        s
    }

    fn refresh_coefficients(&mut self, force: bool) {
        let fs = self.sample_rate as f32;
        let lo = self.params.eq_low_db();
        let mi = self.params.eq_mid_db();
        let hi = self.params.eq_high_db();
        let f = self.params.filter();
        let room = self.params.reverb_room();

        if force || (lo - self.cached_eq_low).abs() > 0.01 {
            self.cached_eq_low = lo;
            let co = low_shelf(fs, EQ_LOW_HZ, EQ_Q, lo);
            for f in &mut self.eq_low {
                f.update_coefficients(co);
            }
        }
        if force || (mi - self.cached_eq_mid).abs() > 0.01 {
            self.cached_eq_mid = mi;
            let co = peaking_eq(fs, EQ_MID_HZ, EQ_Q, mi);
            for f in &mut self.eq_mid {
                f.update_coefficients(co);
            }
        }
        if force || (hi - self.cached_eq_high).abs() > 0.01 {
            self.cached_eq_high = hi;
            let co = high_shelf(fs, EQ_HIGH_HZ, EQ_Q, hi);
            for f in &mut self.eq_high {
                f.update_coefficients(co);
            }
        }
        if force || (f - self.cached_filter).abs() > 0.001 {
            self.cached_filter = f;
            let coeffs = filter_coefficients(fs, f);
            for slot in &mut self.filter_state {
                match (slot.as_mut(), coeffs) {
                    (Some(state), Some(c)) => state.update_coefficients(c),
                    (None, Some(c)) => *slot = Some(DirectForm1::<f32>::new(c)),
                    (Some(_), None) => *slot = None,
                    (None, None) => {}
                }
            }
        }
        if force || (room - self.cached_reverb_room).abs() > 0.01 {
            self.cached_reverb_room = room;
            for r in &mut self.reverb {
                r.set_room(room);
            }
        }
    }
}

fn filter_coefficients(fs: f32, pos: f32) -> Option<Coefficients<f32>> {
    let pos = pos.clamp(-1.0, 1.0);
    if pos.abs() < 0.02 {
        return None; // bypass
    }
    if pos < 0.0 {
        // LPF
        let intensity = -pos;
        let cutoff = 80.0 + (22000.0 - 80.0) * (1.0 - intensity).powi(3);
        Coefficients::<f32>::from_params(
            Type::LowPass,
            fs.hz(),
            cutoff.hz(),
            Q_BUTTERWORTH_F32,
        )
        .ok()
    } else {
        // HPF
        let intensity = pos;
        let cutoff = 30.0 + (15000.0 - 30.0) * intensity.powi(2);
        Coefficients::<f32>::from_params(
            Type::HighPass,
            fs.hz(),
            cutoff.hz(),
            Q_BUTTERWORTH_F32,
        )
        .ok()
    }
}

impl<S> Iterator for DjEffectSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let s = self.inner.next()?;

        let n_ch = self.channels as usize;
        let ch = (self.current_channel as usize) % n_ch;
        self.current_channel = self.current_channel.wrapping_add(1);

        self.sample_count = self.sample_count.wrapping_add(1);
        if self.sample_count % PARAM_REFRESH_INTERVAL == 0 {
            self.refresh_coefficients(false);
        }

        // EQ chain
        let mut s = self.eq_low[ch].run(s);
        s = self.eq_mid[ch].run(s);
        s = self.eq_high[ch].run(s);

        // HPF/LPF combined filter（バイパス時は None）
        if let Some(filter) = self.filter_state[ch].as_mut() {
            s = filter.run(s);
        }

        // Echo
        let echo_wet = self.params.echo_wet();
        if echo_wet > 0.001 {
            let echo_time = self.params.echo_time_ms();
            let echo_fb = self.params.echo_feedback();
            s = self.echo[ch].process(s, echo_time, echo_fb, echo_wet);
        } else {
            // バッファだけ進めて状態を保つ（無音のtail を残さない）
            self.echo[ch].process(s, 100.0, 0.0, 0.0);
        }

        // Reverb（dry/wet ミックス）
        let reverb_wet = self.params.reverb_wet();
        if reverb_wet > 0.001 {
            let wet_signal = self.reverb[ch].process(s);
            s = s * (1.0 - reverb_wet) + wet_signal * reverb_wet;
        }

        Some(s)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S> Source for DjEffectSource<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
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
}
