//! Audio worker and bounded transfer to CPAL. Callbacks only touch fixed-size
//! frames, lock-free queues and atomics; decoding and DSP run off the callback.
use crate::{
    config::{AudioOutputConfig, AudioOutputStatus, MixingMode},
    deck::{LoopState, TempoRange},
    dsp::{DspParams, StereoProcessor},
    pcm::PcmTrack,
    voice::{Voice, VoiceControl},
};
use crossbeam::queue::ArrayQueue;
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(crate) const BUFFER_FRAMES: usize = 512;
const MAX_RENDER_FRAMES: usize = 16384;
pub(crate) const CUE_BUFFER_FRAMES: usize = 32768;
#[derive(Clone)]
pub(crate) struct DeckControl {
    pub track: Option<Arc<PcmTrack>>,
    pub generation: u64,
    pub position: f64,
    pub playing: bool,
    pub volume: f32,
    pub effective_volume: f32,
    pub cue_send: f32,
    pub tempo_range: TempoRange,
    pub tempo_adjust: f32,
    pub sync_speed: Option<f32>,
    pub nudge: f32,
    pub key_lock: bool,
    pub pitch: f32,
    pub loops: LoopState,
    pub dsp: Arc<DspParams>,
    pub transport_cue: f64,
    pub cue_held: bool,
    pub cue_was_playing: bool,
    pub jog_touch: bool,
    pub jog_velocity: f32,
    pub jog_events: u64,
}
impl Default for DeckControl {
    fn default() -> Self {
        Self {
            track: None,
            generation: 0,
            position: 0.0,
            playing: false,
            volume: 1.0,
            effective_volume: 1.0,
            cue_send: 0.0,
            tempo_range: TempoRange::default(),
            tempo_adjust: 0.0,
            sync_speed: None,
            nudge: 0.0,
            key_lock: false,
            pitch: 0.0,
            loops: LoopState::default(),
            dsp: DspParams::new_arc(),
            transport_cue: 0.0,
            cue_held: false,
            cue_was_playing: false,
            jog_touch: false,
            jog_velocity: 0.0,
            jog_events: 0,
        }
    }
}
impl DeckControl {
    pub fn speed(&self) -> f32 {
        (self
            .sync_speed
            .unwrap_or(1.0 + self.tempo_adjust * self.tempo_range.max_adjust())
            + self.nudge)
            .clamp(0.25, 4.0)
    }
}
#[derive(Clone)]
pub(crate) struct Controls {
    pub decks: [DeckControl; 2],
    pub config: AudioOutputConfig,
}
#[derive(Clone, Copy, Default)]
pub(crate) struct RenderFrame {
    pub buses: [[f32; 2]; 4],
    pub positions: [f64; 2],
    pub generations: [u64; 2],
    pub main_gains: [f32; 2],
    pub cue_gains: [f32; 2],
    pub headphone_mix: f32,
    pub headphone_volume: f32,
}
#[derive(Clone, Copy, Default)]
pub(crate) struct CueFrame {
    pub samples: [f32; 2],
    pub generations: [u64; 2],
    pub contributions: [[f32; 2]; 2],
}
impl CueFrame {
    fn audible(self, shared: &Shared) -> [f32; 2] {
        let valid: [bool; 2] = std::array::from_fn(|i| {
            self.generations[i] == shared.generations[i].load(Ordering::Acquire)
        });
        if valid[0] && valid[1] {
            return self.samples;
        }
        std::array::from_fn(|ch| {
            ((if valid[0] {
                self.contributions[0][ch]
            } else {
                0.0
            }) + (if valid[1] {
                self.contributions[1][ch]
            } else {
                0.0
            }))
            .clamp(-1.0, 1.0)
        })
    }
}
pub(crate) struct Shared {
    pub controls: Mutex<Controls>,
    pub frames: ArrayQueue<RenderFrame>,
    pub cue_frames: ArrayQueue<CueFrame>,
    pub positions: [AtomicU64; 2],
    pub position_generations: [AtomicU64; 2],
    pub position_sequences: [AtomicU64; 2],
    pub requested_positions: [AtomicU64; 2],
    pub generations: [AtomicU64; 2],
    pub consumed: AtomicU64,
    pub underruns: AtomicU64,
    pub lost: AtomicBool,
    pub running: AtomicBool,
    pub quit: AtomicBool,
    pub sample_rate: u32,
    pub channels: u16,
    pub peaks: [AtomicU32; 8],
    pub callback_frames: AtomicU64,
    pub render_ahead: AtomicU64,
    pub cue_attached: AtomicBool,
}
impl Shared {
    pub fn new(config: AudioOutputConfig, sample_rate: u32, channels: u16) -> Arc<Self> {
        let render_ahead = (config.buffer_frames.unwrap_or(256) as usize + 256)
            .clamp(BUFFER_FRAMES, MAX_RENDER_FRAMES) as u64;
        Arc::new(Self {
            controls: Mutex::new(Controls {
                decks: std::array::from_fn(|_| DeckControl::default()),
                config,
            }),
            frames: ArrayQueue::new(MAX_RENDER_FRAMES),
            cue_frames: ArrayQueue::new(CUE_BUFFER_FRAMES),
            positions: std::array::from_fn(|_| AtomicU64::new(0f64.to_bits())),
            position_generations: std::array::from_fn(|_| AtomicU64::new(0)),
            position_sequences: std::array::from_fn(|_| AtomicU64::new(0)),
            requested_positions: std::array::from_fn(|_| AtomicU64::new(0f64.to_bits())),
            generations: std::array::from_fn(|_| AtomicU64::new(0)),
            consumed: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            lost: AtomicBool::new(false),
            running: AtomicBool::new(false),
            quit: AtomicBool::new(false),
            sample_rate,
            channels,
            peaks: std::array::from_fn(|_| AtomicU32::new(0f32.to_bits())),
            callback_frames: AtomicU64::new(0),
            render_ahead: AtomicU64::new(render_ahead),
            cue_attached: AtomicBool::new(false),
        })
    }
    pub fn position(&self, index: usize) -> f64 {
        loop {
            let seq = self.position_sequences[index].load(Ordering::Acquire);
            if seq & 1 != 0 {
                std::hint::spin_loop();
                continue;
            }
            let pos = self.positions[index].load(Ordering::Acquire);
            let gen = self.position_generations[index].load(Ordering::Acquire);
            if seq != self.position_sequences[index].load(Ordering::Acquire) {
                continue;
            }
            return f64::from_bits(if gen == self.generations[index].load(Ordering::Acquire) {
                pos
            } else {
                self.requested_positions[index].load(Ordering::Acquire)
            });
        }
    }
    fn commit_position(&self, index: usize, position: f64, generation: u64) {
        self.position_sequences[index].fetch_add(1, Ordering::AcqRel);
        self.positions[index].store(position.to_bits(), Ordering::Release);
        self.position_generations[index].store(generation, Ordering::Release);
        self.position_sequences[index].fetch_add(1, Ordering::Release);
    }
    pub fn transport(&self, index: usize, update: impl FnOnce(&mut DeckControl)) {
        let mut c = self.controls.lock();
        let d = &mut c.decks[index];
        d.position = self.position(index);
        update(d);
        d.generation = d.generation.wrapping_add(1);
        self.requested_positions[index].store(d.position.to_bits(), Ordering::Release);
        self.generations[index].store(d.generation, Ordering::Release);
    }
    pub fn status(&self) -> AudioOutputStatus {
        let p: [f32; 8] =
            std::array::from_fn(|i| f32::from_bits(self.peaks[i].load(Ordering::Relaxed)));
        let lost = self.lost.load(Ordering::Acquire);
        AudioOutputStatus {sample_rate:self.sample_rate,output_channels:self.channels,elapsed_frames:self.consumed.load(Ordering::Relaxed),underruns:self.underruns.load(Ordering::Relaxed),device_lost:lost,error:lost.then(||"Audio output device was lost; playback stopped. Select and reopen an available device.".into()),estimated_latency_ms:1000.0*(BUFFER_FRAMES as f64+self.callback_frames.load(Ordering::Relaxed) as f64)/self.sample_rate as f64,peak_main:[p[0],p[1]],peak_cue:[p[2],p[3]],peak_decks:[[p[4],p[5]],[p[6],p[7]]] }
    }
}
pub(crate) fn start_worker(shared: Arc<Shared>) -> JoinHandle<()> {
    thread::Builder::new()
        .name("conduction-render".into())
        .spawn(move || {
            let initial = shared.controls.lock().clone();
            let mut voices: [Voice; 2] = std::array::from_fn(|_| Voice::new(shared.sample_rate));
            let mut processors: [StereoProcessor; 2] = std::array::from_fn(|i| {
                StereoProcessor::new(shared.sample_rate, initial.decks[i].dsp.clone())
            });
            let mut generations = [u64::MAX; 2];
            let mut jog_events = [0; 2];
            let mut jog_life = [0usize; 2];
            while !shared.quit.load(Ordering::Acquire) {
                if shared.lost.load(Ordering::Acquire) {
                    let mut c = shared.controls.lock();
                    for d in &mut c.decks {
                        d.playing = false;
                        d.jog_touch = false;
                        d.cue_held = false;
                        d.nudge = 0.0;
                    }
                    drop(c);
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                let available = (shared.render_ahead.load(Ordering::Relaxed) as usize)
                    .saturating_sub(shared.frames.len());
                if available < 64 {
                    thread::sleep(Duration::from_micros(500));
                    continue;
                }
                let c = shared.controls.lock().clone();
                for i in 0..2 {
                    if c.decks[i].generation != generations[i] {
                        voices[i].reset(c.decks[i].track.clone(), c.decks[i].position);
                        processors[i].reset();
                        generations[i] = c.decks[i].generation;
                    }
                    if c.decks[i].jog_events != jog_events[i] {
                        jog_events[i] = c.decks[i].jog_events;
                        jog_life[i] = (shared.sample_rate / 20) as usize;
                    }
                }
                for _ in 0..available.min(256) {
                    let mut frame = RenderFrame {
                        generations,
                        main_gains: [c.decks[0].effective_volume, c.decks[1].effective_volume],
                        cue_gains: [c.decks[0].cue_send, c.decks[1].cue_send],
                        headphone_mix: c.config.headphone_mix,
                        headphone_volume: c.config.headphone_volume,
                        ..Default::default()
                    };
                    for i in 0..2 {
                        let d = &c.decks[i];
                        let voice_control = VoiceControl {
                            playing: d.playing,
                            speed: d.speed(),
                            key_lock: d.key_lock,
                            pitch_semitones: d.pitch,
                            loop_start: d.loops.start_sec,
                            loop_end: d.loops.end_sec,
                            loop_active: d.loops.active,
                            jog_touch: d.jog_touch,
                            jog_velocity: if jog_life[i] > 0 { d.jog_velocity } else { 0.0 },
                        };
                        jog_life[i] = jog_life[i].saturating_sub(1);
                        let (audio, position, _) = voices[i].render(&voice_control);
                        frame.positions[i] = position;
                        frame.buses[2 + i] =
                            processors[i].process(audio, c.config.mode == MixingMode::External);
                    }
                    for ch in 0..2 {
                        frame.buses[0][ch] = (frame.buses[2][ch] * c.decks[0].effective_volume
                            + frame.buses[3][ch] * c.decks[1].effective_volume)
                            .clamp(-1.0, 1.0);
                        let pfl = frame.buses[2][ch] * c.decks[0].cue_send
                            + frame.buses[3][ch] * c.decks[1].cue_send;
                        frame.buses[1][ch] = (c.config.headphone_volume
                            * (pfl * (1.0 - c.config.headphone_mix)
                                + frame.buses[0][ch] * c.config.headphone_mix))
                            .clamp(-1.0, 1.0);
                    }
                    if shared.frames.push(frame).is_err() {
                        break;
                    }
                }
            }
        })
        .expect("render worker thread")
}

/// One owner in the MAIN callback, no locks or allocations.
pub(crate) struct MainCallback {
    pub shared: Arc<Shared>,
    pub config: AudioOutputConfig,
}
impl MainCallback {
    pub fn write<T: cpal::Sample + cpal::FromSample<f32>>(&mut self, data: &mut [T]) {
        let channels = self.shared.channels as usize;
        let mut peaks = [0f32; 8];
        let mut underrun = false;
        let hardware_frames = data.len() / channels;
        self.shared
            .callback_frames
            .store(hardware_frames as u64, Ordering::Relaxed);
        self.shared.render_ahead.store(
            (hardware_frames + 256).clamp(BUFFER_FRAMES, MAX_RENDER_FRAMES) as u64,
            Ordering::Relaxed,
        );
        for output in data.chunks_mut(channels) {
            for s in output.iter_mut() {
                *s = T::from_sample(0.0);
            }
            if self.shared.lost.load(Ordering::Acquire) {
                continue;
            }
            let Some(mut frame) = self.shared.frames.pop() else {
                underrun = true;
                continue;
            };
            let mut stale = false;
            for i in 0..2 {
                if frame.generations[i] != self.shared.generations[i].load(Ordering::Acquire) {
                    frame.buses[2 + i] = [0.0; 2];
                    stale = true;
                } else {
                    self.shared
                        .commit_position(i, frame.positions[i], frame.generations[i]);
                }
            }
            if stale {
                for ch in 0..2 {
                    frame.buses[0][ch] = (frame.buses[2][ch] * frame.main_gains[0]
                        + frame.buses[3][ch] * frame.main_gains[1])
                        .clamp(-1.0, 1.0);
                    let pfl = frame.buses[2][ch] * frame.cue_gains[0]
                        + frame.buses[3][ch] * frame.cue_gains[1];
                    frame.buses[1][ch] = (frame.headphone_volume
                        * (pfl * (1.0 - frame.headphone_mix)
                            + frame.buses[0][ch] * frame.headphone_mix))
                        .clamp(-1.0, 1.0);
                }
            }
            match self.config.mode {
                MixingMode::Internal => {
                    put_pair(output, self.config.main_pair, frame.buses[0]);
                    if !self.shared.cue_attached.load(Ordering::Relaxed) {
                        if let Some(pair) = self.config.cue_pair {
                            put_pair(output, pair, frame.buses[1]);
                        }
                    }
                }
                MixingMode::External => {
                    put_pair(output, self.config.deck_a_pair, frame.buses[2]);
                    put_pair(output, self.config.deck_b_pair, frame.buses[3]);
                }
            }
            if self.shared.cue_attached.load(Ordering::Relaxed)
                && self
                    .shared
                    .cue_frames
                    .push(CueFrame {
                        samples: frame.buses[1],
                        generations: frame.generations,
                        contributions: std::array::from_fn(|i| {
                            std::array::from_fn(|ch| {
                                frame.buses[2 + i][ch]
                                    * frame.headphone_volume
                                    * (frame.cue_gains[i] * (1.0 - frame.headphone_mix)
                                        + frame.main_gains[i] * frame.headphone_mix)
                            })
                        }),
                    })
                    .is_err()
            {
                underrun = true;
            }
            for (i, bus) in frame.buses.iter().enumerate() {
                for ch in 0..2 {
                    peaks[i * 2 + ch] = peaks[i * 2 + ch].max(bus[ch].abs());
                }
            }
        }
        self.shared
            .consumed
            .fetch_add((data.len() / channels) as u64, Ordering::Relaxed);
        if underrun {
            self.shared.underruns.fetch_add(1, Ordering::Relaxed);
        }
        for (i, peak) in peaks.iter().enumerate() {
            let old = f32::from_bits(self.shared.peaks[i].load(Ordering::Relaxed));
            self.shared.peaks[i].store(peak.max(old * 0.85).to_bits(), Ordering::Relaxed);
        }
    }
}
fn put_pair<T: cpal::Sample + cpal::FromSample<f32>>(data: &mut [T], pair: u16, value: [f32; 2]) {
    if let Some(out) = data.get_mut(pair as usize..pair as usize + 2) {
        out[0] = T::from_sample(value[0].clamp(-1.0, 1.0));
        out[1] = T::from_sample(value[1].clamp(-1.0, 1.0));
    }
}

/// CUE follows MAIN's consumed samples, with a small elastic FIFO and bounded
/// rate correction for the independent hardware clock. No second decoder.
pub(crate) struct CueCallback {
    pub shared: Arc<Shared>,
    pub channels: usize,
    pub pair: u16,
    pub ratio: f64,
    pub phase: f64,
    pub previous: CueFrame,
    pub next: CueFrame,
    pub primed: bool,
}
impl CueCallback {
    pub fn write<T: cpal::Sample + cpal::FromSample<f32>>(&mut self, data: &mut [T]) {
        let target = (self.shared.sample_rate as f64 * 0.015).max(128.0);
        let error = (self.shared.cue_frames.len() as f64 - target) / target;
        let step = self.ratio * (1.0 + (error * 0.001).clamp(-0.005, 0.005));
        let mut underrun = false;
        for out in data.chunks_mut(self.channels) {
            for s in out.iter_mut() {
                *s = T::from_sample(0.0);
            }
            if self.shared.lost.load(Ordering::Acquire) {
                continue;
            }
            if !self.primed {
                if self.shared.cue_frames.len() < target as usize {
                    continue;
                }
                self.previous = self.shared.cue_frames.pop().unwrap_or_default();
                self.next = self.shared.cue_frames.pop().unwrap_or_default();
                self.phase = 0.0;
                self.primed = true;
            }
            let previous = self.previous.audible(&self.shared);
            let next = self.next.audible(&self.shared);
            let value =
                std::array::from_fn(|i| previous[i] + (next[i] - previous[i]) * self.phase as f32);
            put_pair(out, self.pair, value);
            self.phase += step;
            while self.phase >= 1.0 {
                self.phase -= 1.0;
                self.previous = self.next;
                match self.shared.cue_frames.pop() {
                    Some(v) => self.next = v,
                    None => {
                        self.primed = false;
                        underrun = true;
                        break;
                    }
                }
            }
        }
        if underrun {
            self.shared.underruns.fetch_add(1, Ordering::Relaxed);
        }
    }
}
