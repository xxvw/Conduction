//! A worker-owned stereo voice. Its public position advances only when a frame
//! is returned, independently of SoundTouch's input lookahead.

use std::sync::Arc;

use soundtouch::SoundTouch;

use crate::pcm::PcmTrack;

const BLOCK_FRAMES: usize = 256;

#[derive(Debug, Clone)]
pub(crate) struct VoiceControl {
    pub playing: bool,
    pub speed: f32,
    pub key_lock: bool,
    pub pitch_semitones: f32,
    pub loop_start: Option<f64>,
    pub loop_end: Option<f64>,
    pub loop_active: bool,
    pub jog_touch: bool,
    pub jog_velocity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct StretchSettings {
    speed: f64,
    key_lock: bool,
    pitch: f64,
    loop_frames: Option<(f64, f64)>,
}

pub(crate) struct Voice {
    track: Option<Arc<PcmTrack>>,
    output_rate: u32,
    /// Source-frame cursor corresponding to frames already returned.
    position: f64,
    /// Separate cursor for the processor's lookahead input.
    input_position: f64,
    touch: SoundTouch,
    stretch_settings: Option<StretchSettings>,
    pending: [f32; BLOCK_FRAMES * 2],
    pending_frames: usize,
    pending_index: usize,
    pending_speed: f64,
    input_eof: bool,
    flushed: bool,
    fed_input: bool,
}

impl Voice {
    pub(crate) fn new(output_rate: u32) -> Self {
        let output_rate = output_rate.max(1);
        let mut touch = SoundTouch::new();
        touch.set_sample_rate(output_rate).set_channels(2);
        Self {
            track: None,
            output_rate,
            position: 0.0,
            input_position: 0.0,
            touch,
            stretch_settings: None,
            pending: [0.0; BLOCK_FRAMES * 2],
            pending_frames: 0,
            pending_index: 0,
            pending_speed: 1.0,
            input_eof: false,
            flushed: false,
            fed_input: false,
        }
    }

    /// Loading, seeking, and Stop all use the same reset path. A reset also
    /// discards any samples produced for the preceding position or track.
    pub(crate) fn reset(&mut self, track: Option<Arc<PcmTrack>>, position_sec: f64) {
        self.track = track.filter(|track| track.sample_rate > 0 && !track.samples.is_empty());
        self.position = self.track.as_ref().map_or(0.0, |track| {
            finite_or(position_sec, 0.0).max(0.0) * f64::from(track.sample_rate)
        });
        if let Some(track) = &self.track {
            self.position = self.position.min(track.samples.len() as f64);
        }
        self.clear_stretch();
    }

    /// Returns a stereo frame, its resulting source position in seconds, and
    /// whether transport playback remains active. A touched jog may audition a
    /// paused deck without changing that transport state.
    pub(crate) fn render(&mut self, control: &VoiceControl) -> ([f32; 2], f64, bool) {
        let Some(track) = self.track.clone() else {
            return ([0.0; 2], 0.0, false);
        };
        let rate = f64::from(track.sample_rate);
        if !control.playing && !control.jog_touch {
            return ([0.0; 2], self.position / rate, false);
        }

        let loop_frames = loop_frames(control, rate, track.samples.len());
        let speed = finite_or(f64::from(control.speed), 1.0).clamp(0.25, 4.0);
        let pitch = finite_or(f64::from(control.pitch_semitones), 0.0).clamp(-12.0, 12.0);
        let use_stretch = !control.jog_touch && (control.key_lock || pitch.abs() > f64::EPSILON);

        if !use_stretch {
            if self.stretch_settings.is_some() {
                self.clear_stretch();
            }
            let velocity = if control.jog_touch {
                finite_or(f64::from(control.jog_velocity), 0.0).clamp(-16.0, 16.0)
            } else {
                speed
            };
            if velocity == 0.0 {
                return ([0.0; 2], self.position / rate, control.playing);
            }
            let len = track.samples.len() as f64;
            self.position = wrap_position(self.position, velocity, loop_frames);
            if control.jog_touch && velocity < 0.0 && self.position >= len {
                self.position = len - 1.0;
            }
            if self.position >= len
                || (loop_frames.is_none() && velocity < 0.0 && self.position <= 0.0)
            {
                return (
                    [0.0; 2],
                    self.position / rate,
                    control.jog_touch && control.playing,
                );
            }
            let frame = interpolate(&track, self.position, loop_frames);
            self.position = advance_position(
                self.position,
                velocity * rate / f64::from(self.output_rate),
                len,
                loop_frames,
            );
            let remaining = control.playing
                && (control.jog_touch || loop_frames.is_some() || self.position < len);
            return (frame, self.position / rate, remaining);
        }

        let settings = StretchSettings {
            speed,
            key_lock: control.key_lock,
            pitch,
            loop_frames,
        };
        if !self
            .stretch_settings
            .is_some_and(|active| active.loop_frames == settings.loop_frames)
        {
            // A loop edit or entering stretch mode is a source discontinuity.
            // Re-prime at the audible cursor, not at prefetched input.
            self.clear_stretch();
            self.configure_stretch(settings);
        }

        self.position = wrap_position(self.position, speed, loop_frames);
        if self.position >= track.samples.len() as f64 {
            return ([0.0; 2], self.position / rate, false);
        }
        if self.pending_index == self.pending_frames {
            self.fill_pending(&track, settings);
        }
        if self.pending_index == self.pending_frames {
            if self.flushed {
                // SoundTouch may round its final frame count. Never repeatedly
                // flush an exhausted processor or keep an EOF voice running.
                self.position = track.samples.len() as f64;
                return ([0.0; 2], self.position / rate, false);
            }
            // A finite priming budget prevents an unexpected processor state
            // from monopolizing the worker. Input cursor progress is retained.
            return ([0.0; 2], self.position / rate, true);
        }

        let index = self.pending_index * 2;
        let frame = [self.pending[index], self.pending[index + 1]];
        self.pending_index += 1;
        self.position = advance_position(
            self.position,
            self.pending_speed * rate / f64::from(self.output_rate),
            track.samples.len() as f64,
            loop_frames,
        );
        (
            frame,
            self.position / rate,
            loop_frames.is_some() || self.position < track.samples.len() as f64,
        )
    }

    fn clear_stretch(&mut self) {
        self.touch.clear();
        self.stretch_settings = None;
        self.input_position = self.position;
        self.pending_frames = 0;
        self.pending_index = 0;
        self.pending_speed = 1.0;
        self.input_eof = false;
        self.flushed = false;
        self.fed_input = false;
    }

    fn configure_stretch(&mut self, settings: StretchSettings) {
        self.touch
            .set_tempo(if settings.key_lock {
                settings.speed
            } else {
                1.0
            })
            .set_rate(if settings.key_lock {
                1.0
            } else {
                settings.speed
            })
            .set_pitch(2.0_f64.powf(settings.pitch / 12.0));
        self.stretch_settings = Some(settings);
    }

    fn fill_pending(&mut self, track: &PcmTrack, settings: StretchSettings) {
        self.pending_index = 0;
        self.pending_frames = 0;
        let loop_frames = settings.loop_frames;
        let input_step = f64::from(track.sample_rate) / f64::from(self.output_rate);
        // At most one second of new input per call, in bounded stack buffers.
        // This comfortably covers SoundTouch's analysis window at every usual
        // hardware sample rate, including its initial lookahead.
        let budget = (self.output_rate as usize / BLOCK_FRAMES).clamp(8, 4096);
        let mut input = [0.0; BLOCK_FRAMES * 2];
        for _ in 0..=budget {
            self.pending_frames = self.touch.receive_samples(&mut self.pending, BLOCK_FRAMES);
            self.pending_speed = self
                .stretch_settings
                .map_or(settings.speed, |active| active.speed);
            if self.pending_frames > 0 || self.flushed {
                return;
            }
            if self.input_eof {
                if self.fed_input && self.touch.num_unprocessed_samples() > 0 {
                    self.touch.flush();
                }
                self.flushed = true;
                // Drain even on the last budget iteration, with no extra feed.
                self.pending_frames = self.touch.receive_samples(&mut self.pending, BLOCK_FRAMES);
                return;
            }
            // First drain every frame generated with the old parameters. Then
            // update the streaming processor without destroying its overlap
            // history on every MIDI tempo movement or Link phase correction.
            if self.stretch_settings != Some(settings) {
                self.configure_stretch(settings);
            }
            let mut frames = 0;
            while frames < BLOCK_FRAMES {
                self.input_position = wrap_position(self.input_position, 1.0, loop_frames);
                if self.input_position >= track.samples.len() as f64 {
                    self.input_eof = true;
                    break;
                }
                let frame = interpolate(track, self.input_position, loop_frames);
                input[frames * 2] = frame[0];
                input[frames * 2 + 1] = frame[1];
                frames += 1;
                self.input_position = advance_position(
                    self.input_position,
                    input_step,
                    track.samples.len() as f64,
                    loop_frames,
                );
            }
            if frames > 0 {
                self.touch.put_samples(&input[..frames * 2], frames);
                self.fed_input = true;
            }
        }
    }
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn loop_frames(control: &VoiceControl, rate: f64, len: usize) -> Option<(f64, f64)> {
    if !control.loop_active {
        return None;
    }
    let (start, end) = control.loop_start.zip(control.loop_end)?;
    if !start.is_finite() || !end.is_finite() {
        return None;
    }
    let start = (start * rate).round().clamp(0.0, len as f64);
    let end = (end * rate).round().clamp(0.0, len as f64);
    (end > start).then_some((start, end))
}

fn wrap_position(position: f64, direction: f64, loop_frames: Option<(f64, f64)>) -> f64 {
    if let Some((start, end)) = loop_frames {
        if (direction >= 0.0 && position >= end) || (direction < 0.0 && position < start) {
            return start + (position - start).rem_euclid(end - start);
        }
    }
    position
}

fn advance_position(position: f64, step: f64, len: f64, loop_frames: Option<(f64, f64)>) -> f64 {
    let next = wrap_position(position + step, step, loop_frames);
    if loop_frames.is_none() && next >= len - 1e-7 {
        len
    } else {
        next.clamp(0.0, len)
    }
}

fn interpolate(track: &PcmTrack, position: f64, loop_frames: Option<(f64, f64)>) -> [f32; 2] {
    let index = (position.floor() as usize).min(track.samples.len() - 1);
    let fraction = (position - index as f64) as f32;
    let mut next = (index + 1).min(track.samples.len() - 1);
    if let Some((start, end)) = loop_frames {
        if index as f64 + 1.0 >= end {
            next = start as usize;
        }
    }
    let a = track.samples[index];
    let b = track.samples[next];
    [
        a[0] + (b[0] - a[0]) * fraction,
        a[1] + (b[1] - a[1]) * fraction,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control() -> VoiceControl {
        VoiceControl {
            playing: true,
            speed: 1.0,
            key_lock: false,
            pitch_semitones: 0.0,
            loop_start: None,
            loop_end: None,
            loop_active: false,
            jog_touch: false,
            jog_velocity: 0.0,
        }
    }

    fn track(rate: u32, samples: Vec<[f32; 2]>) -> Arc<PcmTrack> {
        Arc::new(PcmTrack {
            sample_rate: rate,
            samples: samples.into(),
        })
    }

    fn ramp(rate: u32, frames: usize) -> Arc<PcmTrack> {
        track(rate, (0..frames).map(|i| [i as f32, -(i as f32)]).collect())
    }

    #[test]
    fn stereo_linear_resampling_and_sample_clock() {
        let mut voice = Voice::new(8);
        voice.reset(Some(ramp(4, 8)), 0.0);
        let c = control();
        assert_eq!(voice.render(&c), ([0.0, 0.0], 0.125, true));
        assert_eq!(voice.render(&c), ([0.5, -0.5], 0.25, true));
        assert_eq!(voice.render(&c), ([1.0, -1.0], 0.375, true));
    }

    #[test]
    fn speed_changes_advance_once_per_output_frame() {
        let mut voice = Voice::new(8);
        voice.reset(Some(ramp(8, 64)), 0.0);
        let mut c = control();
        assert_eq!(voice.render(&c).1, 0.125);
        c.speed = 2.0;
        assert_eq!(voice.render(&c).1, 0.375);
        c.speed = 0.5;
        assert_eq!(voice.render(&c).1, 0.4375);
        c.playing = false;
        assert_eq!(voice.render(&c), ([0.0; 2], 0.4375, false));
        c.playing = true;
        assert_eq!(voice.render(&c).1, 0.5);
    }

    #[test]
    fn stop_and_reset_retain_replayable_track() {
        let pcm = ramp(4, 4);
        let mut voice = Voice::new(4);
        voice.reset(Some(pcm.clone()), 0.0);
        let c = control();
        for _ in 0..4 {
            voice.render(&c);
        }
        assert_eq!(voice.render(&c), ([0.0; 2], 1.0, false));
        voice.reset(Some(pcm), 0.0);
        assert_eq!(voice.render(&c), ([0.0, 0.0], 0.25, true));
        assert_eq!(voice.render(&c), ([1.0, -1.0], 0.5, true));
        voice.reset(None, 0.0);
        assert_eq!(voice.render(&c), ([0.0; 2], 0.0, false));
    }

    #[test]
    fn loop_wraps_at_the_output_sample_including_interpolation() {
        let mut voice = Voice::new(8);
        voice.reset(Some(ramp(4, 8)), 0.75);
        let mut c = control();
        c.loop_start = Some(0.25);
        c.loop_end = Some(1.0);
        c.loop_active = true;
        assert_eq!(voice.render(&c), ([3.0, -3.0], 0.875, true));
        assert_eq!(voice.render(&c), ([2.0, -2.0], 0.25, true));
        assert_eq!(voice.render(&c), ([1.0, -1.0], 0.375, true));
    }

    #[test]
    fn jog_holds_and_scratches_backwards_without_starting_transport() {
        let mut voice = Voice::new(4);
        voice.reset(Some(ramp(4, 8)), 1.0);
        let mut c = control();
        c.playing = false;
        c.jog_touch = true;
        assert_eq!(voice.render(&c), ([0.0; 2], 1.0, false));
        c.jog_velocity = -1.0;
        assert_eq!(voice.render(&c), ([4.0, -4.0], 0.75, false));
        assert_eq!(voice.render(&c), ([3.0, -3.0], 0.5, false));
        c.jog_touch = false;
        assert_eq!(voice.render(&c), ([0.0; 2], 0.5, false));
    }

    #[test]
    fn reverse_scratch_wraps_a_loop_starting_at_frame_zero() {
        let mut voice = Voice::new(4);
        voice.reset(Some(ramp(4, 8)), 0.25);
        let mut c = control();
        c.jog_touch = true;
        c.jog_velocity = -1.0;
        c.loop_start = Some(0.0);
        c.loop_end = Some(1.0);
        c.loop_active = true;
        assert_eq!(voice.render(&c), ([1.0, -1.0], 0.0, true));
        assert_eq!(voice.render(&c), ([0.0, 0.0], 0.75, true));
        assert_eq!(voice.render(&c), ([3.0, -3.0], 0.5, true));
    }

    #[test]
    fn key_lock_clock_does_not_include_lookahead_or_double_speed() {
        let rate = 48_000;
        let mut voice = Voice::new(rate);
        voice.reset(
            Some(track(rate, vec![[0.25, -0.25]; rate as usize * 2])),
            0.0,
        );
        let mut c = control();
        c.key_lock = true;
        c.speed = 1.5;
        let mut last = 0.0;
        let mut peak = 0.0_f32;
        for _ in 0..4_800 {
            let (frame, position, playing) = voice.render(&c);
            assert!(playing);
            assert!((position - last - 1.5 / f64::from(rate)).abs() < 1e-10);
            assert!((frame[0] + frame[1]).abs() < 1e-5);
            peak = peak.max(frame[0].abs());
            last = position;
        }
        assert!((last - 0.15).abs() < 1e-9);
        assert!(peak > 0.1);
        c.key_lock = false;
        c.pitch_semitones = 3.5;
        assert!((voice.render(&c).1 - last - 1.5 / f64::from(rate)).abs() < 1e-10);
    }

    #[test]
    fn short_stretched_track_finishes_without_repeated_flush() {
        let mut voice = Voice::new(48_000);
        voice.reset(Some(track(48_000, vec![[0.25, -0.25]; 12])), 0.0);
        let mut c = control();
        c.key_lock = true;
        let mut finished = false;
        for _ in 0..100 {
            if !voice.render(&c).2 {
                finished = true;
                break;
            }
        }
        assert!(finished);
        for _ in 0..10 {
            assert_eq!(voice.render(&c), ([0.0; 2], 12.0 / 48_000.0, false));
        }
    }

    #[test]
    fn streaming_tempo_change_preserves_audio_and_consumed_clock() {
        let rate = 48_000;
        let mut voice = Voice::new(rate);
        voice.reset(
            Some(track(rate, vec![[0.25, -0.25]; rate as usize * 2])),
            0.0,
        );
        let mut c = control();
        c.key_lock = true;
        c.speed = 1.5;
        let mut position = 0.0;
        for _ in 0..4_800 {
            position = voice.render(&c).1;
        }
        c.speed = 2.0;
        let mut received_new_speed = false;
        for _ in 0..9_600 {
            let (frame, next, playing) = voice.render(&c);
            assert!(playing);
            assert!(frame[0] > 0.1, "tempo update inserted silence");
            let consumed = (next - position) * f64::from(rate);
            assert!((consumed - 1.5).abs() < 1e-8 || (consumed - 2.0).abs() < 1e-8);
            received_new_speed |= (consumed - 2.0).abs() < 1e-8;
            position = next;
        }
        assert!(received_new_speed);
    }

    #[test]
    fn malformed_controls_are_bounded() {
        let mut voice = Voice::new(4);
        voice.reset(Some(ramp(4, 8)), f64::NAN);
        let mut c = control();
        c.speed = f32::NAN;
        c.pitch_semitones = f32::NAN;
        c.loop_active = true;
        c.loop_start = Some(f64::NEG_INFINITY);
        c.loop_end = Some(f64::NAN);
        assert_eq!(voice.render(&c), ([0.0, 0.0], 0.25, true));
    }
}
