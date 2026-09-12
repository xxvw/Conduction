use crate::{
    device::OutputDevice,
    dsp::DspParams,
    error::{AudioError, AudioResult},
    pcm::PcmTrack,
    runtime::Shared,
};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
pub const CHANNEL_VOLUME_MIN: f32 = 0.0;
pub const CHANNEL_VOLUME_MAX: f32 = 2.0;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeckId {
    A,
    B,
}
impl DeckId {
    pub(crate) fn index(self) -> usize {
        match self {
            Self::A => 0,
            Self::B => 1,
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum TempoRange {
    #[default]
    Six,
    Ten,
    Sixteen,
}
impl TempoRange {
    pub fn max_adjust(self) -> f32 {
        match self {
            Self::Six => 0.06,
            Self::Ten => 0.10,
            Self::Sixteen => 0.16,
        }
    }
    pub fn as_percent(self) -> u8 {
        match self {
            Self::Six => 6,
            Self::Ten => 10,
            Self::Sixteen => 16,
        }
    }
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct LoopState {
    pub start_sec: Option<f64>,
    pub end_sec: Option<f64>,
    pub active: bool,
}
/// Control handle for one voice in the device's common renderer. Playback
/// position is committed by the MAIN callback after it consumes each frame.
pub struct Deck {
    id: DeckId,
    pub(crate) shared: Arc<Shared>,
}
impl Deck {
    #[cfg(test)]
    pub(crate) fn offline(id: DeckId, shared: Arc<Shared>) -> Self {
        Self { id, shared }
    }
    pub fn new(
        id: DeckId,
        device: &OutputDevice,
        cue_device: Option<&OutputDevice>,
    ) -> AudioResult<Self> {
        device.start(cue_device)?;
        Ok(Self {
            id,
            shared: device.shared.clone(),
        })
    }
    pub fn id(&self) -> DeckId {
        self.id
    }
    /// Compatibility decoder; asynchronous callers decode on a worker then use load_pcm.
    pub fn load(
        &mut self,
        _device: &OutputDevice,
        _cue_device: Option<&OutputDevice>,
        path: &Path,
    ) -> AudioResult<()> {
        self.load_pcm(Arc::new(PcmTrack::decode(path)?))
    }
    pub fn load_pcm(&mut self, track: Arc<PcmTrack>) -> AudioResult<()> {
        if track.sample_rate == 0 || track.samples.is_empty() {
            return Err(AudioError::Decode(
                "empty PCM or invalid sample rate".into(),
            ));
        }
        self.shared.transport(self.id.index(), |d| {
            d.track = Some(track);
            d.position = 0.0;
            d.playing = false;
            d.loops = LoopState::default();
            d.transport_cue = 0.0;
            d.cue_held = false;
            d.cue_was_playing = false;
            d.jog_touch = false;
            d.jog_velocity = 0.0;
            d.nudge = 0.0;
        });
        Ok(())
    }
    pub fn play(&mut self) {
        if self.is_playing() {
            let mut controls = self.shared.controls.lock();
            let deck = &mut controls.decks[self.id.index()];
            if deck.cue_held {
                deck.cue_was_playing = true;
            }
            return;
        }
        if self.shared.lost.load(Ordering::Acquire) {
            return;
        }
        self.shared.transport(self.id.index(), |d| {
            if let Some(track) = &d.track {
                if d.position >= track.duration().as_secs_f64() {
                    d.position = 0.0;
                }
                d.playing = true;
                if d.cue_held {
                    d.cue_was_playing = true;
                }
            }
        });
    }
    pub fn pause(&mut self) {
        if !self.shared.controls.lock().decks[self.id.index()].playing {
            return;
        }
        self.shared.transport(self.id.index(), |d| {
            d.playing = false;
            d.jog_velocity = 0.0;
        });
    }
    pub fn stop(&mut self) {
        self.shared.transport(self.id.index(), |d| {
            d.playing = false;
            d.position = 0.0;
            d.cue_held = false;
            d.jog_touch = false;
            d.jog_velocity = 0.0;
            d.nudge = 0.0;
        });
    }
    pub fn seek(&mut self, position: Duration) -> AudioResult<()> {
        if self.duration().is_none() {
            return Err(AudioError::Playback("no track loaded".into()));
        }
        self.shared.transport(self.id.index(), |d| {
            d.position = position
                .as_secs_f64()
                .min(d.track.as_ref().unwrap().duration().as_secs_f64());
        });
        Ok(())
    }
    pub fn is_playing(&self) -> bool {
        let c = self.shared.controls.lock();
        let d = &c.decks[self.id.index()];
        d.playing
            && !self.shared.lost.load(Ordering::Acquire)
            && d.track.as_ref().is_some_and(|t| {
                self.position().as_secs_f64() < t.duration().as_secs_f64() || d.loops.active
            })
    }
    pub fn is_paused(&self) -> bool {
        !self.is_playing() && !self.is_finished()
    }
    pub fn is_finished(&self) -> bool {
        self.duration().is_none_or(|d| self.position() >= d)
    }
    pub fn duration(&self) -> Option<Duration> {
        self.shared.controls.lock().decks[self.id.index()]
            .track
            .as_ref()
            .map(|t| t.duration())
    }
    pub fn position(&self) -> Duration {
        Duration::from_secs_f64(self.shared.position(self.id.index()).max(0.0))
    }
    pub fn dsp_params(&self) -> Arc<DspParams> {
        self.shared.controls.lock().decks[self.id.index()]
            .dsp
            .clone()
    }
    pub fn set_channel_volume(&mut self, v: f32) {
        if v.is_finite() {
            self.shared.controls.lock().decks[self.id.index()].volume =
                v.clamp(CHANNEL_VOLUME_MIN, CHANNEL_VOLUME_MAX);
        }
    }
    pub fn channel_volume(&self) -> f32 {
        self.shared.controls.lock().decks[self.id.index()].volume
    }
    pub fn effective_volume(&self) -> f32 {
        self.shared.controls.lock().decks[self.id.index()].effective_volume
    }
    pub fn tempo_range(&self) -> TempoRange {
        self.shared.controls.lock().decks[self.id.index()].tempo_range
    }
    pub fn set_tempo_range(&mut self, range: TempoRange) {
        self.shared.controls.lock().decks[self.id.index()].tempo_range = range;
    }
    pub fn tempo_adjust(&self) -> f32 {
        self.shared.controls.lock().decks[self.id.index()].tempo_adjust
    }
    pub fn set_tempo_adjust(&mut self, pos: f32) {
        if pos.is_finite() {
            let mut c = self.shared.controls.lock();
            let d = &mut c.decks[self.id.index()];
            d.tempo_adjust = pos.clamp(-1.0, 1.0);
            d.sync_speed = None;
        }
    }
    pub fn playback_speed(&self) -> f32 {
        self.shared.controls.lock().decks[self.id.index()].speed()
    }
    pub fn set_sync_speed(&mut self, speed: Option<f32>) {
        if speed.is_some_and(|s| !s.is_finite()) {
            return;
        }
        self.shared.controls.lock().decks[self.id.index()].sync_speed =
            speed.map(|s| s.clamp(0.25, 4.0));
    }
    pub fn key_lock(&self) -> bool {
        self.shared.controls.lock().decks[self.id.index()].key_lock
    }
    pub fn set_key_lock(&mut self, on: bool) {
        self.shared.controls.lock().decks[self.id.index()].key_lock = on;
    }
    pub fn pitch_offset_semitones(&self) -> f32 {
        self.shared.controls.lock().decks[self.id.index()].pitch
    }
    pub fn set_pitch_offset_semitones(&mut self, semitones: f32) {
        if semitones.is_finite() {
            self.shared.controls.lock().decks[self.id.index()].pitch = semitones.clamp(-12.0, 12.0);
        }
    }
    pub fn cue_send(&self) -> f32 {
        self.shared.controls.lock().decks[self.id.index()].cue_send
    }
    pub fn has_cue_output(&self) -> bool {
        let c = self.shared.controls.lock();
        c.config.cue_pair.is_some() || self.shared.cue_attached.load(Ordering::Acquire)
    }
    pub fn set_cue_send(&mut self, value: f32) {
        if value.is_finite() {
            self.shared.controls.lock().decks[self.id.index()].cue_send = value.clamp(0.0, 1.0);
        }
    }
    pub fn loop_state(&self) -> LoopState {
        self.shared.controls.lock().decks[self.id.index()].loops
    }
    pub fn set_loop_in(&mut self, sec: f64) {
        if !sec.is_finite() {
            return;
        }
        let mut c = self.shared.controls.lock();
        let d = &mut c.decks[self.id.index()];
        let sec = sec.max(0.0).min(
            d.track
                .as_ref()
                .map_or(sec.max(0.0), |t| t.duration().as_secs_f64()),
        );
        d.loops.start_sec = Some(sec);
        if d.loops.end_sec.is_some_and(|e| e <= sec) {
            d.loops.end_sec = None;
            d.loops.active = false;
        }
    }
    pub fn set_loop_out(&mut self, sec: f64) {
        if !sec.is_finite() {
            return;
        }
        let mut c = self.shared.controls.lock();
        let d = &mut c.decks[self.id.index()];
        let sec = sec.max(0.0).min(
            d.track
                .as_ref()
                .map_or(sec.max(0.0), |t| t.duration().as_secs_f64()),
        );
        d.loops.end_sec = Some(sec);
        d.loops.active = d.loops.start_sec.is_some_and(|s| s < sec);
    }
    pub fn toggle_loop(&mut self) {
        let mut c = self.shared.controls.lock();
        let d = &mut c.decks[self.id.index()];
        if d.loops
            .start_sec
            .zip(d.loops.end_sec)
            .is_some_and(|(s, e)| s < e)
        {
            d.loops.active = !d.loops.active;
        }
    }
    pub fn clear_loop(&mut self) {
        self.shared.controls.lock().decks[self.id.index()].loops = LoopState::default();
    }
    /// Loops are now applied inside the renderer at source sample boundaries.
    pub fn process_loop(&mut self) -> AudioResult<()> {
        Ok(())
    }
    pub fn transport_cue_held(&self) -> bool {
        self.shared.controls.lock().decks[self.id.index()].cue_held
    }
    pub fn transport_cue(&self) -> f64 {
        self.shared.controls.lock().decks[self.id.index()].transport_cue
    }
    pub fn set_transport_cue(&mut self, sec: f64) {
        if sec.is_finite() {
            let mut c = self.shared.controls.lock();
            let d = &mut c.decks[self.id.index()];
            d.transport_cue = sec
                .max(0.0)
                .min(d.track.as_ref().map_or(0.0, |t| t.duration().as_secs_f64()));
        }
    }
    pub fn transport_cue_press(&mut self) {
        if self.shared.controls.lock().decks[self.id.index()].cue_held {
            return;
        }
        self.shared.transport(self.id.index(), |d| {
            if d.track.is_none() {
                return;
            }
            d.cue_was_playing = d.playing;
            d.position = d.transport_cue;
            d.playing = !d.cue_was_playing;
            d.cue_held = true;
        });
    }
    pub fn transport_cue_release(&mut self) {
        let held = self.shared.controls.lock().decks[self.id.index()].cue_held;
        if held {
            self.shared.transport(self.id.index(), |d| {
                if !d.cue_was_playing {
                    d.position = d.transport_cue;
                    d.playing = false;
                }
                d.cue_held = false;
            });
        }
    }
    pub fn set_jog_touch(&mut self, touch: bool) {
        if self.shared.controls.lock().decks[self.id.index()].jog_touch == touch {
            return;
        }
        self.shared.transport(self.id.index(), |d| {
            d.jog_touch = touch;
            d.jog_velocity = 0.0;
        });
    }
    pub fn jog(&mut self, delta_seconds: f64) {
        if !delta_seconds.is_finite() {
            return;
        }
        let held = self.shared.controls.lock().decks[self.id.index()].jog_touch;
        if held {
            let mut c = self.shared.controls.lock();
            let d = &mut c.decks[self.id.index()];
            d.jog_velocity = (delta_seconds / 0.05).clamp(-8.0, 8.0) as f32;
            d.jog_events = d.jog_events.wrapping_add(1);
        } else {
            self.shared.transport(self.id.index(), |d| {
                d.position = (d.position + delta_seconds)
                    .max(0.0)
                    .min(d.track.as_ref().map_or(0.0, |t| t.duration().as_secs_f64()));
            });
        }
    }
    pub fn set_nudge(&mut self, value: f32) {
        if value.is_finite() {
            self.shared.controls.lock().decks[self.id.index()].nudge = value.clamp(-0.25, 0.25);
        }
    }
    pub fn release_momentary_controls(&mut self) {
        self.transport_cue_release();
        self.set_jog_touch(false);
        self.set_nudge(0.0);
    }
}
