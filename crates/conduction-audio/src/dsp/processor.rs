//! Stereo frame DSP for the shared renderer, with no allocation while processing.
//!
//! An external mixer owns EQ and the channel filter; deck echo and reverb remain
//! active in either routing mode. Delay lines are allocated only at construction.

use std::sync::Arc;

use biquad::{Biquad, Coefficients, DirectForm1, ToHertz, Type, Q_BUTTERWORTH_F32};

use super::coefficients::{high_shelf, low_shelf, peaking_eq};
use super::echo::EchoEffect;
use super::reverb::SchroederReverb;
use super::DspParams;

const PARAM_REFRESH_FRAMES: u32 = 32;
const EQ_Q: f32 = 0.7;

#[derive(Clone, Copy)]
struct Settings {
    low: f32,
    mid: f32,
    high: f32,
    filter: f32,
    echo_wet: f32,
    echo_time: f32,
    echo_feedback: f32,
    reverb_wet: f32,
    room: f32,
}

impl Settings {
    fn read(params: &DspParams) -> Self {
        Self {
            low: finite(params.eq_low_db(), 0.0).clamp(-60.0, 12.0),
            mid: finite(params.eq_mid_db(), 0.0).clamp(-60.0, 12.0),
            high: finite(params.eq_high_db(), 0.0).clamp(-60.0, 12.0),
            filter: finite(params.filter(), 0.0).clamp(-1.0, 1.0),
            echo_wet: finite(params.echo_wet(), 0.0).clamp(0.0, 1.0),
            echo_time: finite(params.echo_time_ms(), 375.0).clamp(10.0, 2400.0),
            echo_feedback: finite(params.echo_feedback(), 0.4).clamp(0.0, 0.92),
            reverb_wet: finite(params.reverb_wet(), 0.0).clamp(0.0, 1.0),
            room: finite(params.reverb_room(), 0.5).clamp(0.0, 1.0),
        }
    }
}

fn finite(value: f32, default: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        default
    }
}

pub(crate) struct StereoProcessor {
    params: Arc<DspParams>,
    sample_rate: f32,
    eq_low: [DirectForm1<f32>; 2],
    eq_mid: [DirectForm1<f32>; 2],
    eq_high: [DirectForm1<f32>; 2],
    filter: [Option<DirectForm1<f32>>; 2],
    echo: [EchoEffect; 2],
    reverb: [SchroederReverb; 2],
    settings: Settings,
    frames_until_refresh: u32,
    external: bool,
}

impl StereoProcessor {
    pub(crate) fn new(sample_rate: u32, params: Arc<DspParams>) -> Self {
        // CPAL supplies a positive rate. Keep even malformed test/input rates
        // safe for the delay line and constrain all filters below Nyquist.
        let sample_rate = sample_rate.max(2) as f32;
        let settings = Settings::read(&params);
        let frequency = |hz: f32| hz.min(sample_rate * 0.45);
        let low = low_shelf(sample_rate, frequency(250.0), EQ_Q, settings.low);
        let mid = peaking_eq(sample_rate, frequency(1000.0), EQ_Q, settings.mid);
        let high = high_shelf(sample_rate, frequency(4000.0), EQ_Q, settings.high);
        let filter = filter_coefficients(sample_rate, settings.filter);
        let mut reverb = std::array::from_fn(|_| SchroederReverb::new(sample_rate));
        for effect in &mut reverb {
            effect.set_room(settings.room);
        }
        Self {
            params,
            sample_rate,
            eq_low: std::array::from_fn(|_| DirectForm1::new(low)),
            eq_mid: std::array::from_fn(|_| DirectForm1::new(mid)),
            eq_high: std::array::from_fn(|_| DirectForm1::new(high)),
            filter: std::array::from_fn(|_| filter.map(DirectForm1::new)),
            echo: std::array::from_fn(|_| EchoEffect::new(sample_rate)),
            reverb,
            settings,
            frames_until_refresh: 0,
            external: false,
        }
    }

    pub(crate) fn process(&mut self, frame: [f32; 2], external: bool) -> [f32; 2] {
        if self.frames_until_refresh == 0 {
            self.refresh();
            self.frames_until_refresh = PARAM_REFRESH_FRAMES;
        }
        self.frames_until_refresh -= 1;
        if external != self.external {
            self.reset_filters();
            self.external = external;
        }

        let settings = self.settings;
        let mut output = [0.0; 2];
        for (channel, sample) in frame.into_iter().enumerate() {
            let mut sample = sample;
            if !external {
                sample = self.eq_low[channel].run(sample);
                sample = self.eq_mid[channel].run(sample);
                sample = self.eq_high[channel].run(sample);
                if let Some(filter) = &mut self.filter[channel] {
                    sample = filter.run(sample);
                }
            }
            sample = self.echo[channel].process(
                sample,
                settings.echo_time,
                settings.echo_feedback,
                settings.echo_wet,
            );
            // Advance the reverb even when dry so turning the effect back on
            // never resumes a frozen tail from an earlier passage.
            let reverberated = self.reverb[channel].process(sample);
            output[channel] =
                sample * (1.0 - settings.reverb_wet) + reverberated * settings.reverb_wet;
        }
        output
    }

    /// Clear seek/load discontinuity history while preserving current controls.
    pub(crate) fn reset(&mut self) {
        self.reset_filters();
        for effect in &mut self.echo {
            effect.reset();
        }
        for effect in &mut self.reverb {
            effect.reset();
        }
        self.frames_until_refresh = 0;
    }

    fn reset_filters(&mut self) {
        for filter in self
            .eq_low
            .iter_mut()
            .chain(self.eq_mid.iter_mut())
            .chain(self.eq_high.iter_mut())
        {
            filter.reset_state();
        }
        for filter in self.filter.iter_mut().flatten() {
            filter.reset_state();
        }
    }

    fn refresh(&mut self) {
        let next = Settings::read(&self.params);
        let rate = self.sample_rate;
        if next.low != self.settings.low {
            let coefficients = low_shelf(rate, 250.0_f32.min(rate * 0.45), EQ_Q, next.low);
            for filter in &mut self.eq_low {
                filter.update_coefficients(coefficients);
            }
        }
        if next.mid != self.settings.mid {
            let coefficients = peaking_eq(rate, 1000.0_f32.min(rate * 0.45), EQ_Q, next.mid);
            for filter in &mut self.eq_mid {
                filter.update_coefficients(coefficients);
            }
        }
        if next.high != self.settings.high {
            let coefficients = high_shelf(rate, 4000.0_f32.min(rate * 0.45), EQ_Q, next.high);
            for filter in &mut self.eq_high {
                filter.update_coefficients(coefficients);
            }
        }
        if next.filter != self.settings.filter {
            let coefficients = filter_coefficients(rate, next.filter);
            for slot in &mut self.filter {
                match (slot.as_mut(), coefficients) {
                    (Some(filter), Some(coefficients)) => filter.update_coefficients(coefficients),
                    (_, coefficients) => *slot = coefficients.map(DirectForm1::new),
                }
            }
        }
        if next.room != self.settings.room {
            for reverb in &mut self.reverb {
                reverb.set_room(next.room);
            }
        }
        self.settings = next;
    }
}

fn filter_coefficients(sample_rate: f32, position: f32) -> Option<Coefficients<f32>> {
    if position.abs() < 0.02 {
        return None;
    }
    let (kind, cutoff) = if position < 0.0 {
        (Type::LowPass, 80.0 + 21920.0 * (1.0 + position).powi(3))
    } else {
        (Type::HighPass, 30.0 + 14970.0 * position.powi(2))
    };
    Coefficients::from_params(
        kind,
        sample_rate.hz(),
        cutoff.min(sample_rate * 0.45).hz(),
        Q_BUTTERWORTH_F32,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_routing_bypasses_eq_and_filter() {
        let params = DspParams::new_arc();
        params.set_eq_low_db(-48.0);
        params.set_eq_mid_db(-36.0);
        params.set_eq_high_db(-24.0);
        params.set_filter(0.9);
        let mut internal = StereoProcessor::new(48000, Arc::clone(&params));
        let mut external = StereoProcessor::new(48000, params);
        let mut internal_energy = 0.0;
        let mut dry_energy = 0.0;
        for i in 0..4800 {
            let value = (i as f32 * std::f32::consts::TAU * 1000.0 / 48000.0).sin() * 0.5;
            let input = [value, -value];
            assert_eq!(external.process(input, true), input);
            internal_energy += internal.process(input, false)[0].powi(2);
            dry_energy += value.powi(2);
        }
        assert!(internal_energy < dry_energy * 0.01);
    }

    #[test]
    fn external_routing_retains_echo_without_stereo_crosstalk() {
        let params = DspParams::new_arc();
        params.set_echo_time_ms(10.0);
        params.set_echo_wet(0.5);
        params.set_echo_feedback(0.0);
        let mut processor = StereoProcessor::new(48000, params);
        assert_eq!(processor.process([1.0, 0.0], true), [0.5, 0.0]);
        for _ in 1..480 {
            assert_eq!(processor.process([0.0; 2], true), [0.0; 2]);
        }
        assert_eq!(processor.process([0.0; 2], true), [0.5, 0.0]);
    }

    #[test]
    fn external_routing_retains_reverb_and_reset_removes_tails() {
        let params = DspParams::new_arc();
        params.set_reverb_wet(1.0);
        let mut processor = StereoProcessor::new(48000, params);
        processor.process([0.0, 1.0], true);
        let mut tail_energy = 0.0;
        for _ in 0..4800 {
            let frame = processor.process([0.0; 2], true);
            assert_eq!(frame[0], 0.0);
            tail_energy += frame[1].powi(2);
        }
        assert!(tail_energy > 0.01);
        processor.reset();
        for _ in 0..4800 {
            assert_eq!(processor.process([0.0; 2], true), [0.0; 2]);
        }
    }

    #[test]
    fn reset_removes_echo_history_and_refreshes_controls() {
        let params = DspParams::new_arc();
        params.set_echo_wet(1.0);
        params.set_echo_time_ms(10.0);
        let mut processor = StereoProcessor::new(48000, Arc::clone(&params));
        processor.process([1.0; 2], false);
        processor.reset();
        for _ in 0..1000 {
            assert_eq!(processor.process([0.0; 2], false), [0.0; 2]);
        }
        params.set_echo_wet(0.0);
        processor.reset();
        assert_eq!(processor.process([0.25, -0.25], true), [0.25, -0.25]);
    }
}
