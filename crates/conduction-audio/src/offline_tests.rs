//! No output device is opened here: feed the production worker and callbacks
//! with deterministic PCM, including independently clocked MAIN/CUE streams.

use std::sync::{atomic::Ordering, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::config::{AudioOutputConfig, MixingMode};
use crate::deck::{Deck, DeckId};
use crate::pcm::PcmTrack;
use crate::runtime::{start_worker, CueCallback, MainCallback, RenderFrame, Shared};

const RATE: u32 = 48_000;

struct Worker {
    shared: Arc<Shared>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    fn start(shared: &Arc<Shared>) -> Self {
        Self {
            shared: shared.clone(),
            handle: Some(start_worker(shared.clone())),
        }
    }

    fn ready(&self, frames: usize) {
        wait_until(|| self.shared.frames.len() >= frames);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shared.quit.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "audio worker did not make progress"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn pcm(samples: [f32; 2]) -> Arc<PcmTrack> {
    Arc::new(PcmTrack {
        sample_rate: RATE,
        samples: vec![samples; RATE as usize * 2].into(),
    })
}

fn main_callback(shared: &Arc<Shared>) -> MainCallback {
    MainCallback {
        shared: shared.clone(),
        config: shared.controls.lock().config.clone(),
    }
}

fn cue_callback(shared: &Arc<Shared>, rate: u32) -> CueCallback {
    CueCallback {
        shared: shared.clone(),
        channels: 4,
        pair: 2,
        ratio: f64::from(shared.sample_rate) / f64::from(rate),
        phase: 0.0,
        previous: Default::default(),
        next: Default::default(),
        primed: false,
    }
}

fn enqueue(shared: &Shared, frame: RenderFrame) {
    assert!(shared.frames.push(frame).is_ok(), "render queue is full");
}

fn near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-5,
        "sample {actual} does not match {expected}"
    );
}

#[test]
fn main_callback_routes_stereo_buses_without_channel_leakage() {
    let config = AudioOutputConfig {
        main_pair: 2,
        cue_pair: Some(4),
        ..Default::default()
    };
    let shared = Shared::new(config, RATE, 8);
    let mut callback = main_callback(&shared);
    enqueue(
        &shared,
        RenderFrame {
            buses: [[0.1, -0.2], [0.3, -0.4], [0.7; 2], [0.9; 2]],
            positions: [1.5, 2.0],
            ..Default::default()
        },
    );
    let mut output = [1.0_f32; 8];
    callback.write(&mut output);
    assert_eq!(output, [0.0, 0.0, 0.1, -0.2, 0.3, -0.4, 0.0, 0.0]);
    assert_eq!(shared.position(0), 1.5);
    assert_eq!(shared.position(1), 2.0);
    assert_eq!(shared.status().peak_main, [0.1, 0.2]);
    assert_eq!(shared.status().peak_cue, [0.3, 0.4]);
}

#[test]
fn external_callback_sends_independent_decks_and_ignores_internal_buses() {
    let config = AudioOutputConfig {
        mode: MixingMode::External,
        deck_a_pair: 4,
        deck_b_pair: 0,
        ..Default::default()
    };
    let shared = Shared::new(config, RATE, 6);
    enqueue(
        &shared,
        RenderFrame {
            buses: [[0.0; 2], [0.9; 2], [0.25, -0.5], [-0.75, 0.4]],
            ..Default::default()
        },
    );
    let mut output = [1.0_f32; 6];
    main_callback(&shared).write(&mut output);
    assert_eq!(output, [-0.75, 0.4, 0.0, 0.0, 0.25, -0.5]);
}

#[test]
fn underrun_outputs_silence_without_advancing_source_positions() {
    let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
    enqueue(
        &shared,
        RenderFrame {
            positions: [12.5, 3.0],
            ..Default::default()
        },
    );
    let mut callback = main_callback(&shared);
    callback.write(&mut [0.0_f32; 2]);
    let mut output = [1.0_f32; 128];
    callback.write(&mut output);
    assert_eq!(output, [0.0; 128]);
    assert_eq!([shared.position(0), shared.position(1)], [12.5, 3.0]);
    assert_eq!(shared.status().underruns, 1);
    // The hardware clock still elapsed, even though no track samples played.
    assert_eq!(shared.status().elapsed_frames, 65);
}

#[test]
fn transport_generation_discards_old_deck_without_interrupting_other_deck() {
    let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
    enqueue(
        &shared,
        RenderFrame {
            buses: [[0.9; 2], [0.0; 2], [0.9; 2], [0.4, -0.2]],
            main_gains: [0.5, 0.25],
            positions: [45.0, 1.0],
            ..Default::default()
        },
    );
    shared.transport(0, |deck| deck.position = 4.0);
    let mut callback = main_callback(&shared);
    let mut output = [1.0_f32; 2];
    callback.write(&mut output);
    assert_eq!(output, [0.1, -0.05]);
    assert_eq!(shared.position(0), 4.0);
    assert_eq!(shared.position(1), 1.0);
    enqueue(
        &shared,
        RenderFrame {
            buses: [[0.25; 2]; 4],
            positions: [4.01, 0.01],
            generations: [1, 0],
            ..Default::default()
        },
    );
    callback.write(&mut output);
    assert_eq!(output, [0.25; 2]);
    assert_eq!(shared.position(0), 4.01);
}

#[test]
fn lost_device_silences_both_callbacks_and_preserves_position() {
    let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
    shared.cue_attached.store(true, Ordering::Release);
    let mut main = main_callback(&shared);
    feed_main(&shared, &mut main, 960);
    let mut cue = cue_callback(&shared, RATE);
    cue.write(&mut [0.0_f32; 4]);
    assert!(cue.primed);
    let position = shared.position(0);
    let consumed_cue = shared.cue_frames.len();
    shared.lost.store(true, Ordering::Release);
    let mut main_output = [1.0_f32; 64];
    let mut cue_output = [1.0_f32; 128];
    main.write(&mut main_output);
    cue.write(&mut cue_output);
    assert!(main_output.iter().all(|sample| *sample == 0.0));
    assert!(cue_output.iter().all(|sample| *sample == 0.0));
    assert_eq!(shared.position(0), position);
    assert_eq!(shared.cue_frames.len(), consumed_cue);
    assert!(shared.status().device_lost);
    assert!(shared.status().error.is_some());
}

#[test]
fn worker_mixes_main_and_prefader_cue_with_headphone_balance() {
    let config = AudioOutputConfig {
        cue_pair: Some(2),
        headphone_mix: 0.25,
        headphone_volume: 0.8,
        ..Default::default()
    };
    let shared = Shared::new(config, RATE, 4);
    {
        let mut controls = shared.controls.lock();
        let a = &mut controls.decks[0];
        a.track = Some(pcm([0.4, -0.2]));
        a.playing = true;
        a.effective_volume = 0.0;
        a.cue_send = 1.0;
        let b = &mut controls.decks[1];
        b.track = Some(pcm([0.2, 0.6]));
        b.playing = true;
        b.effective_volume = 0.5;
        b.cue_send = 0.0;
    }
    let worker = Worker::start(&shared);
    worker.ready(256);
    // Worker lookahead must not move the public source clock.
    assert_eq!(shared.position(0), 0.0);
    assert_eq!(shared.position(1), 0.0);
    let mut output = [0.0_f32; 256 * 4];
    main_callback(&shared).write(&mut output);
    for frame in output.chunks_exact(4) {
        for (actual, expected) in frame.iter().zip([0.1, 0.3, 0.26, -0.06]) {
            near(*actual, expected);
        }
    }
    assert!((shared.position(0) - 256.0 / f64::from(RATE)).abs() < 1e-10);
    assert_eq!(shared.position(0), shared.position(1));
    assert_eq!(shared.status().underruns, 0);
}

#[test]
fn external_worker_bypasses_channel_eq_filter_and_fader_gain() {
    let config = AudioOutputConfig {
        mode: MixingMode::External,
        deck_a_pair: 0,
        deck_b_pair: 2,
        ..Default::default()
    };
    let shared = Shared::new(config, RATE, 4);
    {
        let mut controls = shared.controls.lock();
        for (deck, sample) in controls.decks.iter_mut().zip([[0.3, -0.4], [-0.2, 0.6]]) {
            deck.track = Some(pcm(sample));
            deck.playing = true;
            deck.volume = 0.0;
            deck.effective_volume = 0.0;
            deck.dsp.set_eq_low_db(-60.0);
            deck.dsp.set_eq_mid_db(-60.0);
            deck.dsp.set_eq_high_db(-60.0);
            deck.dsp.set_filter(1.0);
        }
    }
    let worker = Worker::start(&shared);
    worker.ready(256);
    let mut output = [0.0_f32; 256 * 4];
    main_callback(&shared).write(&mut output);
    for frame in output.chunks_exact(4) {
        assert_eq!(frame, [0.3, -0.4, -0.2, 0.6]);
    }
}

#[test]
fn key_locked_worker_clock_advances_only_for_consumed_frames() {
    let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
    {
        let mut controls = shared.controls.lock();
        controls.decks[0].track = Some(pcm([0.25, -0.25]));
        controls.decks[0].playing = true;
        controls.decks[0].key_lock = true;
        controls.decks[0].sync_speed = Some(1.5);
    }
    let worker = Worker::start(&shared);
    worker.ready(512);
    assert_eq!(shared.position(0), 0.0);
    let mut callback = main_callback(&shared);
    let mut peak = 0.0_f32;
    for _ in 0..20 {
        worker.ready(128);
        let mut output = [0.0_f32; 128 * 2];
        callback.write(&mut output);
        for frame in output.chunks_exact(2) {
            near(frame[0], -frame[1]);
            peak = peak.max(frame[0].abs());
        }
    }
    assert!(peak > 0.1);
    assert!((shared.position(0) - 2560.0 * 1.5 / f64::from(RATE)).abs() < 1e-9);
    assert_eq!(shared.status().underruns, 0);
}

#[test]
fn deck_stop_retains_track_and_load_clears_loop_and_momentary_controls() {
    let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
    let mut deck = Deck::offline(DeckId::A, shared.clone());
    deck.load_pcm(pcm([0.25, -0.25])).unwrap();
    deck.set_loop_in(0.1);
    deck.set_loop_out(0.2);
    deck.play();
    deck.seek(Duration::from_secs_f64(0.15)).unwrap();
    deck.stop();
    assert_eq!(deck.position(), Duration::ZERO);
    assert!(!deck.is_playing());
    assert!(deck.duration().is_some());
    deck.play();
    assert!(deck.is_playing());
    let worker = Worker::start(&shared);
    worker.ready(256);
    let mut output = [0.0_f32; 2];
    main_callback(&shared).write(&mut output);
    near(output[0], 0.25);
    assert!((deck.position().as_secs_f64() - 1.0 / f64::from(RATE)).abs() < 1e-9);
    deck.set_transport_cue(0.15);
    deck.set_nudge(0.1);
    deck.set_jog_touch(true);
    deck.load_pcm(pcm([0.5; 2])).unwrap();
    assert_eq!(deck.position(), Duration::ZERO);
    assert!(!deck.is_playing());
    assert!(!deck.loop_state().active);
    assert_eq!(deck.loop_state().start_sec, None);
    assert_eq!(deck.loop_state().end_sec, None);
    assert_eq!(deck.transport_cue(), 0.0);
    let controls = shared.controls.lock();
    assert!(!controls.decks[0].jog_touch);
    assert!(!controls.decks[0].cue_held);
    assert_eq!(controls.decks[0].nudge, 0.0);
}

#[test]
fn worker_stops_transport_after_device_loss() {
    let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
    {
        let mut controls = shared.controls.lock();
        controls.decks[0].track = Some(pcm([0.5; 2]));
        controls.decks[0].playing = true;
        controls.decks[0].cue_held = true;
        controls.decks[0].jog_touch = true;
        controls.decks[0].nudge = 0.1;
    }
    let worker = Worker::start(&shared);
    worker.ready(256);
    shared.lost.store(true, Ordering::Release);
    wait_until(|| !shared.controls.lock().decks[0].playing);
    let controls = shared.controls.lock();
    assert!(!controls.decks[0].cue_held);
    assert!(!controls.decks[0].jog_touch);
    assert_eq!(controls.decks[0].nudge, 0.0);
}

// Supply separate-CUE data exclusively through the production MAIN callback.
fn feed_main(shared: &Shared, callback: &mut MainCallback, frames: usize) {
    for chunk in (0..frames).step_by(256) {
        let count = (frames - chunk).min(256);
        let initial = shared.position(0);
        for index in 0..count {
            enqueue(
                shared,
                RenderFrame {
                    buses: [[0.1; 2], [0.5, -0.25], [0.0; 2], [0.0; 2]],
                    positions: [initial + (index + 1) as f64 / f64::from(RATE), 0.0],
                    ..Default::default()
                },
            );
        }
        let mut output = vec![0.0_f32; count * shared.channels as usize];
        callback.write(&mut output);
    }
}

#[test]
fn separate_cue_follows_main_samples_with_rate_conversion_and_drift_correction() {
    let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
    shared.cue_attached.store(true, Ordering::Release);
    let mut main = main_callback(&shared);
    let mut cue = cue_callback(&shared, 44_100);
    feed_main(&shared, &mut main, 960);
    // Simulate 10-second playback: 48 kHz MAIN, a 44.1 kHz CUE clock
    // running about 450 ppm fast, and different callback buffer sizes.
    for cycle in 0..1000 {
        feed_main(&shared, &mut main, 480);
        let frames = 441 + usize::from(cycle % 5 == 0);
        let mut output = vec![0.0_f32; frames * 4];
        let position = shared.position(0);
        cue.write(&mut output);
        assert_eq!(shared.position(0), position, "CUE changed MAIN transport");
        for frame in output.chunks_exact(4) {
            assert_eq!(frame, [0.0, 0.0, 0.5, -0.25]);
        }
        assert!(shared.cue_frames.len() < 2000, "clock drift grew the queue");
    }
    assert!(cue.primed);
    assert!(shared.cue_frames.len() > 100);
    assert_eq!(shared.status().underruns, 0);
    assert!((shared.position(0) - 10.02).abs() < 1e-8);
}

#[test]
fn cue_elastic_fifo_consumes_faster_when_occupancy_is_high() {
    fn consumed(initial: usize) -> usize {
        let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
        shared.cue_attached.store(true, Ordering::Release);
        feed_main(&shared, &mut main_callback(&shared), initial);
        cue_callback(&shared, RATE).write(&mut [0.0_f32; 512 * 4]);
        assert_eq!(shared.status().underruns, 0);
        initial - shared.cue_frames.len()
    }
    let near_target = consumed(800);
    let congested = consumed(6000);
    assert!((513..=515).contains(&near_target));
    assert!(congested > near_target);
    assert!(
        congested <= 518,
        "independent clock correction is unbounded"
    );
}

#[test]
fn separate_cue_invalidates_buffered_and_interpolated_audio_per_deck() {
    for headphone_mix in [0.0, 0.4, 1.0] {
        let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
        shared.cue_attached.store(true, Ordering::Release);
        let mut main = main_callback(&shared);
        let headphone_volume = 0.8;
        let deck_a = [0.4, -0.3];
        let deck_b = [0.2, 0.5];
        let main_gains = [0.5, 0.8];
        let cue_gains = [0.75, 0.25];
        let main_samples =
            std::array::from_fn(|ch| deck_a[ch] * main_gains[0] + deck_b[ch] * main_gains[1]);
        let cue_samples = std::array::from_fn(|ch| {
            headphone_volume
                * ((deck_a[ch] * cue_gains[0] + deck_b[ch] * cue_gains[1]) * (1.0 - headphone_mix)
                    + main_samples[ch] * headphone_mix)
        });
        // All cue samples are produced by MAIN before the discontinuity, so
        // invalidation must cover the FIFO and both interpolator endpoints.
        for _ in 0..4 {
            for _ in 0..240 {
                enqueue(
                    &shared,
                    RenderFrame {
                        buses: [main_samples, cue_samples, deck_a, deck_b],
                        main_gains,
                        cue_gains,
                        headphone_mix,
                        headphone_volume,
                        ..Default::default()
                    },
                );
            }
            main.write(&mut [0.0_f32; 240 * 2]);
        }
        let mut cue = cue_callback(&shared, 44_100);
        let mut first = [0.0_f32; 4];
        cue.write(&mut first);
        assert!(cue.primed);
        near(first[2], cue_samples[0]);
        near(first[3], cue_samples[1]);
        shared.transport(0, |deck| deck.position = 1.0);
        let mut output = [0.0_f32; 64 * 4];
        cue.write(&mut output);
        for frame in output.chunks_exact(4) {
            assert_eq!(&frame[..2], &[0.0; 2]);
            for channel in 0..2 {
                let remaining_b = headphone_volume
                    * deck_b[channel]
                    * (cue_gains[1] * (1.0 - headphone_mix) + main_gains[1] * headphone_mix);
                near(frame[2 + channel], remaining_b);
            }
        }
        // Stopping the other deck silences the remaining buffered samples.
        shared.transport(1, |deck| deck.position = 0.0);
        cue.write(&mut output);
        assert!(output.iter().all(|sample| *sample == 0.0));
        assert_eq!(shared.status().underruns, 0);
    }
}

#[test]
fn repeated_transport_cue_press_does_not_toggle_preview() {
    for initially_playing in [false, true] {
        let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
        let mut deck = Deck::offline(DeckId::A, shared);
        deck.load_pcm(pcm([0.25; 2])).unwrap();
        deck.set_transport_cue(0.2);
        deck.seek(Duration::from_secs_f64(0.5)).unwrap();
        if initially_playing {
            deck.play();
        }
        deck.transport_cue_press();
        assert_eq!(deck.is_playing(), !initially_playing);
        assert_eq!(deck.position(), Duration::from_millis(200));
        deck.transport_cue_press();
        assert_eq!(deck.is_playing(), !initially_playing);
        deck.transport_cue_release();
        assert!(!deck.is_playing());
        assert_eq!(deck.position(), Duration::from_millis(200));
        deck.transport_cue_release();
        assert!(!deck.is_playing());
    }
}

#[test]
fn large_hardware_buffer_is_fully_primed_before_callback_consumption() {
    let config = AudioOutputConfig {
        buffer_frames: Some(4096),
        ..Default::default()
    };
    let shared = Shared::new(config, RATE, 2);
    let mut deck = Deck::offline(DeckId::A, shared.clone());
    deck.load_pcm(pcm([0.25, -0.25])).unwrap();
    deck.play();
    let worker = Worker::start(&shared);
    worker.ready(4352);
    assert_eq!(deck.position(), Duration::ZERO);
    let mut callback = main_callback(&shared);
    let mut output = vec![0.0_f32; 4096 * 2];
    callback.write(&mut output);
    for frame in output.chunks_exact(2) {
        near(frame[0], 0.25);
        near(frame[1], -0.25);
    }
    assert_eq!(shared.status().underruns, 0);
    assert_eq!(shared.status().elapsed_frames, 4096);
    assert!((shared.position(0) - 4096.0 / f64::from(RATE)).abs() < 1e-10);
}

#[test]
fn play_latches_a_held_transport_cue_preview() {
    let shared = Shared::new(AudioOutputConfig::default(), RATE, 2);
    let mut deck = Deck::offline(DeckId::A, shared.clone());
    deck.load_pcm(Arc::new(PcmTrack {
        samples: vec![[0.25, -0.25]; RATE as usize].into(),
        sample_rate: RATE,
    }))
    .unwrap();
    deck.transport_cue_press();
    assert!(deck.is_playing());
    deck.play();
    deck.transport_cue_release();
    assert!(deck.is_playing());
    assert!(!deck.transport_cue_held());
}
