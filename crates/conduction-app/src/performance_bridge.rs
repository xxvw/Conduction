//! Off-callback bridge between confirmed audio state, MIDI feedback and Link clocks.

use std::path::Path;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use conduction_audio::DeckId;
use conduction_core::TrackId;
use conduction_link::{LinkEvent, LinkHandle, LocalClock};
use conduction_midi::{Control, ControllerState, MidiDeckState, MidiService};
use crossbeam::channel::{self, Sender};
use parking_lot::Mutex;

use crate::audio_engine::{AudioCommand, AudioHandle, DeckSnapshot, MixerSnapshot, TrackMetadata};
use crate::library_state::LibraryHandle;

const POLL_INTERVAL: Duration = Duration::from_millis(20);
const MASTER_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_EVENTS_PER_TICK: usize = 256;
const METADATA_INTERVAL: Duration = Duration::from_secs(1);

/// Own this for the lifetime of the performance service. Dropping it wakes and joins
/// the worker, so app shutdown cannot leave another snapshot reader running.
pub struct BridgeGuard {
    stop: Sender<()>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for BridgeGuard {
    fn drop(&mut self) {
        let _ = self.stop.try_send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn spawn_bridge(
    audio: AudioHandle,
    library: LibraryHandle,
    link: Arc<Mutex<Option<LinkHandle>>>,
    midi: Arc<MidiService>,
    last_error: Arc<Mutex<Option<String>>>,
) -> BridgeGuard {
    let (stop, stopped) = channel::bounded(1);
    let worker_error = last_error.clone();
    let worker = thread::Builder::new()
        .name("performance-bridge".into())
        .spawn(move || {
            let mut remote = RemoteClock::default();
            let mut track_cache = TrackCache::default();
            let mut previous_config = None;
            let mut metadata_cache = [MetadataCache::default(), MetadataCache::default()];
            let mut next_metadata_refresh = Instant::now();
            loop {
                let snapshot = audio.snapshot();
                let now = Instant::now();
                if now >= next_metadata_refresh {
                    refresh_metadata(
                        &audio,
                        &library,
                        &snapshot,
                        &mut metadata_cache,
                        &worker_error,
                    );
                    next_metadata_refresh = now + METADATA_INTERVAL;
                }
                if let Err(error) = midi.update_state(controller_state(&snapshot)) {
                    record_error(&worker_error, format!("MIDI feedback: {error}"));
                }
                // Configuration may stop/join a network service while holding this
                // mutex. Never make the clock worker or its Drop wait on that lock.
                if let Some(link_guard) = link.try_lock() {
                    if let Some(handle) = link_guard.as_ref() {
                        let config = handle.config();
                        let source_b = config.source_deck == "B";
                        let source = if source_b {
                            &snapshot.deck_b
                        } else {
                            &snapshot.deck_a
                        };
                        let settings = (source_b, config.latency_ms);
                        if previous_config != Some(settings) {
                            send(
                                &audio,
                                AudioCommand::SetSyncLatency {
                                    milliseconds: config.latency_ms,
                                },
                                &worker_error,
                            );
                            send(
                                &audio,
                                AudioCommand::SetMasterDeck {
                                    deck: if source_b { DeckId::B } else { DeckId::A },
                                },
                                &worker_error,
                            );
                            previous_config = Some(settings);
                        }
                        let track_id = track_cache.resolve(
                            source.track_id.as_deref(),
                            &library,
                            &worker_error,
                        );
                        handle.publish_clock(local_clock(source, track_id));

                        let mut beats = Vec::new();
                        for _ in 0..MAX_EVENTS_PER_TICK {
                            let Some(event) = handle.recv_event() else {
                                break;
                            };
                            match event {
                                LinkEvent::Beat {
                                    device_number,
                                    bpm,
                                    received_at_micros,
                                    ..
                                } => beats.push((device_number, bpm, received_at_micros)),
                                LinkEvent::SyncCommand { enabled } => send(
                                    &audio,
                                    AudioCommand::SetSync {
                                        deck: if source_b { DeckId::B } else { DeckId::A },
                                        enabled,
                                        source: "link".into(),
                                    },
                                    &worker_error,
                                ),
                                LinkEvent::Error { message } => {
                                    record_error(&worker_error, message)
                                }
                                LinkEvent::Status { .. } | LinkEvent::MasterChanged { .. } => {}
                            }
                        }
                        // A queued MasterChanged can describe an older owner than
                        // the current snapshot. Select from the latest state once,
                        // then forward only that master's newest valid sample.
                        let latest = handle.snapshot();
                        let master = latest.master_number.filter(|_| latest.running);
                        if remote.select_master(master, latest.player_number) {
                            disconnect(&audio, &worker_error);
                        }
                        if let Some(clock) =
                            remote.accept_batch(beats, unix_micros(), Instant::now())
                        {
                            send(
                                &audio,
                                AudioCommand::UpdateLinkClock {
                                    bpm: clock.bpm,
                                    beat_phase: clock.phase,
                                    connected: true,
                                },
                                &worker_error,
                            );
                        }
                    } else {
                        if remote.select_master(None, None) {
                            disconnect(&audio, &worker_error);
                        }
                        previous_config = None;
                    }
                }
                if remote.expire(Instant::now()) {
                    disconnect(&audio, &worker_error);
                }
                // Audio host extrapolates each new sample using output frames.
                // Re-sending old samples here would hide a lost network master.
                if stopped.recv_timeout(POLL_INTERVAL) != Err(channel::RecvTimeoutError::Timeout) {
                    if remote.connected {
                        disconnect(&audio, &worker_error);
                    }
                    break;
                }
            }
        });
    let worker = match worker {
        Ok(worker) => Some(worker),
        Err(error) => {
            record_error(
                &last_error,
                format!("Cannot start performance bridge: {error}"),
            );
            None
        }
    };
    BridgeGuard { stop, worker }
}

fn send(audio: &AudioHandle, command: AudioCommand, errors: &Mutex<Option<String>>) {
    if let Err(error) = audio.send(command) {
        record_error(errors, error.to_string());
    }
}

fn disconnect(audio: &AudioHandle, errors: &Mutex<Option<String>>) {
    send(
        audio,
        AudioCommand::UpdateLinkClock {
            bpm: 0.0,
            beat_phase: 0.0,
            connected: false,
        },
        errors,
    );
}

fn record_error(errors: &Mutex<Option<String>>, message: String) {
    let mut error = errors.lock();
    if error.as_ref() != Some(&message) {
        *error = Some(message);
    }
}

fn unix_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64
}

/// A complete cached grid also catches reanalysis which changes beat locations
/// without changing BPM. Only two currently loaded tracks are retained.
#[derive(Default)]
struct MetadataCache {
    path: Option<String>,
    generation: u64,
    sent: Option<TrackMetadata>,
}

impl MetadataCache {
    fn needs_refresh(&self, deck: &DeckSnapshot, metadata: &TrackMetadata) -> bool {
        if deck.loading || deck.loaded_path.is_none() || metadata.track_id.is_none() {
            return false;
        }
        self.path != deck.loaded_path
            || self.generation != deck.load_generation
            || self
                .sent
                .as_ref()
                .is_none_or(|sent| !same_metadata(sent, metadata))
            || deck.track_id != metadata.track_id
            || deck.original_bpm != metadata.bpm
            || deck.hot_cues != metadata.hot_cues
    }

    fn mark_sent(&mut self, deck: &DeckSnapshot, metadata: TrackMetadata) {
        self.path = deck.loaded_path.clone();
        self.generation = deck.load_generation;
        self.sent = Some(metadata);
    }
}

fn same_metadata(left: &TrackMetadata, right: &TrackMetadata) -> bool {
    left.track_id == right.track_id
        && left.bpm == right.bpm
        && left.beats == right.beats
        && left.hot_cues == right.hot_cues
}

fn refresh_metadata(
    audio: &AudioHandle,
    library: &LibraryHandle,
    snapshot: &MixerSnapshot,
    cache: &mut [MetadataCache; 2],
    errors: &Mutex<Option<String>>,
) {
    let shared = library.shared();
    let Some(library) = shared.try_lock() else {
        return;
    };
    for (index, (id, deck)) in [(DeckId::A, &snapshot.deck_a), (DeckId::B, &snapshot.deck_b)]
        .into_iter()
        .enumerate()
    {
        // Loading increments the generation before replacing the old path. Do
        // not cache that transitional pair, including when the load later fails.
        if deck.loading {
            continue;
        }
        let Some(path) = deck.loaded_path.as_deref() else {
            continue;
        };
        match read_metadata(&library, Path::new(path)) {
            Ok(Some(metadata)) if cache[index].needs_refresh(deck, &metadata) => {
                // First successful observation always refreshes the grid: the
                // public snapshot cannot prove its full grid matches the DB.
                // Keep the DB guard through enqueue so a later Hot Cue write
                // cannot be followed by this older metadata from the same load.
                match audio.send(AudioCommand::RefreshTrackMetadata {
                    deck: id,
                    path: path.into(),
                    load_generation: deck.load_generation,
                    metadata: metadata.clone(),
                }) {
                    Ok(()) => cache[index].mark_sent(deck, metadata),
                    Err(error) => record_error(errors, format!("Track metadata refresh: {error}")),
                }
            }
            Err(error) => record_error(errors, format!("Track metadata lookup: {error}")),
            // A deleted/unregistered library row is not an instruction to erase
            // the metadata of a track that is already playing. Retry next time.
            Ok(_) => {}
        }
    }
}

fn read_metadata(
    library: &conduction_library::Library,
    path: &Path,
) -> anyhow::Result<Option<TrackMetadata>> {
    let Some(track) = library.get_track_by_path(path)? else {
        return Ok(None);
    };
    let beats = library
        .load_beatgrid(track.id)?
        .into_iter()
        .map(|beat| beat.position_sec)
        .collect();
    let mut hot_cues = vec![None; 8];
    for (slot, position) in library.list_hot_cues(track.id)? {
        if (1..=8).contains(&slot) {
            hot_cues[usize::from(slot - 1)] = Some(position);
        }
    }
    Ok(Some(TrackMetadata {
        track_id: Some(track.id.to_string()),
        bpm: (track.bpm > 0.0).then_some(track.bpm),
        beats,
        hot_cues,
    }))
}

#[derive(Default)]
struct TrackCache {
    track: Option<String>,
    resolved: bool,
    link_id: Option<u32>,
}

impl TrackCache {
    fn resolve(
        &mut self,
        track: Option<&str>,
        library: &LibraryHandle,
        errors: &Mutex<Option<String>>,
    ) -> Option<u32> {
        if self.track.as_deref() != track {
            self.track = track.map(str::to_owned);
            self.resolved = false;
            self.link_id = None;
        }
        if self.resolved {
            return self.link_id;
        }
        let track = track?;
        let Ok(id) = uuid::Uuid::parse_str(track) else {
            self.resolved = true;
            return None;
        };
        let shared = library.shared();
        // Retry an occupied DB on a later tick; neither audio nor shutdown waits
        // behind a long import. Each actual lookup happens only on track change.
        let library = shared.try_lock()?;
        self.resolved = true;
        match library.link_track_id(TrackId::from_uuid(id)) {
            Ok(id) => self.link_id = id,
            Err(error) => record_error(errors, format!("Link track ID lookup: {error}")),
        }
        self.link_id
    }
}

fn local_clock(deck: &DeckSnapshot, track_id: Option<u32>) -> LocalClock {
    let position = deck.beat_position.filter(|value| value.is_finite());
    LocalClock {
        deck: deck.id.into(),
        playing: deck.state == "play",
        bpm: deck
            .bpm
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(f64::from)
            .unwrap_or(0.0),
        beat: position
            .map(|beat| (beat.floor() + 1.0).clamp(0.0, f64::from(u32::MAX)) as u32)
            .unwrap_or(0),
        beat_phase: position.map(|beat| beat.rem_euclid(1.0)).unwrap_or(0.0),
        track_id,
        synced: deck.sync_enabled && !deck.sync_lost,
        grid_available: position.is_some(),
    }
}

fn controller_state(snapshot: &MixerSnapshot) -> ControllerState {
    let mut state = ControllerState::default();
    for deck in [&snapshot.deck_a, &snapshot.deck_b] {
        let name = deck.id.to_owned();
        state.decks.insert(
            name.clone(),
            MidiDeckState {
                playing: deck.state == "play",
                cue: deck.cue_pressed,
                sync: deck.sync_enabled && !deck.sync_lost,
                headphones_cue: deck.cue_send > 0.0,
                hot_cues: std::array::from_fn(|slot| {
                    deck.hot_cues.get(slot).is_some_and(|cue| cue.is_some())
                }),
            },
        );
        for (band, db) in [
            ("low", deck.eq_low_db),
            ("mid", deck.eq_mid_db),
            ("high", deck.eq_high_db),
        ] {
            value(
                &mut state,
                Control::Eq {
                    deck: name.clone(),
                    band: band.into(),
                },
                normalize_eq(db),
            );
        }
        value(
            &mut state,
            Control::Filter { deck: name.clone() },
            bipolar(deck.filter),
        );
        value(
            &mut state,
            Control::Tempo { deck: name.clone() },
            bipolar(deck.tempo_adjust),
        );
        value(
            &mut state,
            Control::Fader { deck: name.clone() },
            deck.channel_volume / 2.0,
        );
        for (parameter, wet) in [
            ("echo_wet", deck.echo_wet),
            ("echo_time_ms", (deck.echo_time_ms - 1.0) / 1999.0),
            ("echo_feedback", deck.echo_feedback / 0.95),
            ("reverb_wet", deck.reverb_wet),
            ("reverb_room", deck.reverb_room),
        ] {
            value(
                &mut state,
                Control::Fx {
                    deck: name.clone(),
                    parameter: parameter.into(),
                },
                wet,
            );
        }
    }
    value(&mut state, Control::Master, snapshot.master_volume / 2.0);
    value(
        &mut state,
        Control::Crossfader,
        bipolar(snapshot.crossfader),
    );
    value(
        &mut state,
        Control::HeadphoneVolume,
        snapshot.headphone_volume / 2.0,
    );
    value(&mut state, Control::HeadphoneMix, snapshot.headphone_mix);
    state
}

fn value(state: &mut ControllerState, control: Control, value: f32) {
    if value.is_finite() {
        state.values.insert(control.key(), value.clamp(0.0, 1.0));
    }
}

fn bipolar(value: f32) -> f32 {
    (value + 1.0) / 2.0
}

fn normalize_eq(db: f32) -> f32 {
    if db < 0.0 {
        0.5 + db / 48.0
    } else {
        0.5 + db / 24.0
    }
}

#[derive(Default)]
struct RemoteClock {
    master: Option<u8>,
    sample: Option<RemoteSample>,
    connected: bool,
}

struct RemoteSample {
    received: Instant,
    received_at_micros: u64,
    bpm: f64,
}

struct AudioClock {
    bpm: f32,
    phase: f64,
}

impl RemoteClock {
    fn accept_batch(
        &mut self,
        beats: Vec<(u8, f64, u64)>,
        wall_now: u64,
        now: Instant,
    ) -> Option<AudioClock> {
        let mut latest = None;
        for (device, bpm, received_at_micros) in beats {
            if let Some(clock) = self.accept_beat(device, bpm, received_at_micros, wall_now, now) {
                latest = Some(clock);
            }
        }
        latest
    }

    /// Returns whether a previously usable remote clock was invalidated.
    fn select_master(&mut self, master: Option<u8>, local_player: Option<u8>) -> bool {
        let remote = master.filter(|number| Some(*number) != local_player);
        if self.master == remote {
            return false;
        }
        self.master = remote;
        self.sample = None;
        std::mem::replace(&mut self.connected, false)
    }

    fn accept_beat(
        &mut self,
        device: u8,
        bpm: f64,
        received_at_micros: u64,
        wall_now: u64,
        now: Instant,
    ) -> Option<AudioClock> {
        if self.master != Some(device)
            || !bpm.is_finite()
            || !(20.0..=400.0).contains(&bpm)
            || received_at_micros > wall_now.saturating_add(100_000)
            || self
                .sample
                .as_ref()
                .is_some_and(|last| received_at_micros <= last.received_at_micros)
        {
            return None;
        }
        // Convert UNIX receipt time to an Instant once. Subsequent expiration and
        // phase progression cannot be affected by wall-clock adjustments.
        let age = Duration::from_micros(wall_now.saturating_sub(received_at_micros));
        if age >= master_timeout(bpm) {
            return None;
        }
        self.sample = Some(RemoteSample {
            received: now.checked_sub(age)?,
            received_at_micros,
            bpm,
        });
        self.connected = true;
        self.sample.as_ref().map(|sample| AudioClock {
            bpm: sample.bpm as f32,
            phase: (now.saturating_duration_since(sample.received).as_secs_f64() * sample.bpm
                / 60.0)
                .rem_euclid(1.0),
        })
    }

    fn expire(&mut self, now: Instant) -> bool {
        if self.connected
            && self.sample.as_ref().is_some_and(|sample| {
                now.saturating_duration_since(sample.received) >= master_timeout(sample.bpm)
            })
        {
            self.sample = None;
            self.connected = false;
            return true;
        }
        false
    }
}

fn master_timeout(bpm: f64) -> Duration {
    // The audio host uses the same two-beat grace period. A 20 BPM master
    // legitimately waits three seconds before sending the next beat.
    Duration::from_secs_f64((120.0 / bpm).max(MASTER_TIMEOUT.as_secs_f64()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck_snapshot(id: &'static str) -> DeckSnapshot {
        DeckSnapshot {
            id,
            state: "play",
            loaded_path: Some("/music/test.wav".into()),
            channel_volume: 1.0,
            effective_volume: 1.0,
            tempo_range_percent: 6,
            tempo_adjust: 0.0,
            playback_speed: 1.0,
            position_sec: 1.625,
            duration_sec: Some(60.0),
            loop_start_sec: None,
            loop_end_sec: None,
            loop_active: false,
            eq_low_db: 0.0,
            eq_mid_db: 0.0,
            eq_high_db: 0.0,
            filter: 0.0,
            echo_wet: 0.0,
            echo_time_ms: 375.0,
            echo_feedback: 0.4,
            reverb_wet: 0.0,
            reverb_room: 0.5,
            cue_send: 0.0,
            has_cue_output: true,
            key_lock: false,
            pitch_offset_semitones: 0.0,
            track_id: None,
            loading: false,
            load_generation: 1,
            load_error: None,
            bpm: Some(120.0),
            original_bpm: Some(120.0),
            beat_position: Some(3.25),
            beat_phase: Some(0.25),
            hot_cues: vec![None; 8],
            transport_cue_sec: 0.0,
            cue_pressed: false,
            jog_touched: false,
            nudge: 0.0,
            sync_enabled: false,
            sync_source: None,
            sync_lost: false,
        }
    }

    #[test]
    fn confirmed_playback_state_drives_link_and_midi_feedback() {
        let mut deck_a = deck_snapshot("A");
        deck_a.cue_pressed = true;
        deck_a.cue_send = 1.0;
        deck_a.sync_enabled = true;
        deck_a.hot_cues[7] = Some(5.0);
        deck_a.eq_low_db = -12.0;
        deck_a.echo_feedback = 0.475;
        deck_a.echo_time_ms = 1000.5;
        let mut deck_b = deck_snapshot("B");
        deck_b.state = "paused";
        deck_b.sync_enabled = true;
        deck_b.sync_lost = true;
        let snapshot = MixerSnapshot {
            deck_a,
            deck_b,
            crossfader: -0.5,
            master_volume: 1.5,
            template: None,
            audio: Default::default(),
            audio_config: Default::default(),
            headphone_mix: 0.25,
            headphone_volume: 1.5,
            output_error: None,
        };
        let feedback = controller_state(&snapshot);
        let a = &feedback.decks["A"];
        assert!(a.playing && a.cue && a.sync && a.headphones_cue && a.hot_cues[7]);
        assert!(!a.hot_cues[0]);
        assert!(!feedback.decks["B"].playing);
        assert!(!feedback.decks["B"].sync);
        assert_eq!(feedback.values["eq:A:low"], 0.25);
        assert_eq!(feedback.values["fader:A"], 0.5);
        assert_eq!(feedback.values["tempo:A"], 0.5);
        assert_eq!(feedback.values["crossfader"], 0.25);
        assert_eq!(feedback.values["master"], 0.75);
        assert_eq!(feedback.values["headphone_volume"], 0.75);
        assert_eq!(feedback.values["fx:A:echo_time_ms"], 0.5);
        assert_eq!(feedback.values["fx:A:echo_feedback"], 0.5);
        let clock = local_clock(&snapshot.deck_a, Some(17));
        assert!(clock.playing && clock.synced && clock.grid_available);
        assert_eq!(clock.beat, 4);
        assert_eq!(clock.beat_phase, 0.25);
        assert_eq!(clock.track_id, Some(17));
        assert!(!local_clock(&snapshot.deck_b, None).playing);
    }

    #[test]
    fn queued_beats_from_previous_master_cannot_overwrite_current_clock() {
        let now = Instant::now();
        let mut clock = RemoteClock::default();
        clock.select_master(Some(3), Some(1));
        let sample = clock
            .accept_batch(
                vec![(2, 90.0, 1_100_000), (3, 120.0, 1_000_000)],
                1_125_000,
                now,
            )
            .unwrap();
        assert_eq!(sample.bpm, 120.0);
        assert!((sample.phase - 0.25).abs() < 1e-6);
        assert!(clock
            .accept_batch(vec![(2, 90.0, 1_200_000)], 1_250_000, now)
            .is_none());
        assert!(clock.connected);
        assert_eq!(clock.sample.unwrap().bpm, 120.0);
    }

    #[test]
    fn pickup_values_preserve_eq_unity_and_endpoints() {
        assert_eq!(normalize_eq(-24.0), 0.0);
        assert_eq!(normalize_eq(-12.0), 0.25);
        assert_eq!(normalize_eq(0.0), 0.5);
        assert_eq!(normalize_eq(6.0), 0.75);
        assert_eq!(normalize_eq(12.0), 1.0);
        assert_eq!(bipolar(-1.0), 0.0);
        assert_eq!(bipolar(0.0), 0.5);
        assert_eq!(bipolar(1.0), 1.0);
        let mut state = ControllerState::default();
        value(&mut state, Control::Master, 2.0 / 2.0);
        value(&mut state, Control::HeadphoneMix, 0.7);
        value(&mut state, Control::HeadphoneVolume, f32::NAN);
        assert_eq!(state.values["master"], 1.0);
        assert_eq!(state.values["headphone_mix"], 0.7);
        assert!(!state.values.contains_key("headphone_volume"));
    }

    #[test]
    fn follows_only_remote_master_and_compensates_queued_beat_age() {
        let now = Instant::now();
        let mut clock = RemoteClock::default();
        clock.select_master(Some(2), Some(1));
        assert!(clock
            .accept_beat(3, 120.0, 1_000_000, 1_125_000, now)
            .is_none());
        let audio = clock
            .accept_beat(2, 120.0, 1_000_000, 1_125_000, now)
            .unwrap();
        assert_eq!(audio.bpm, 120.0);
        assert!((audio.phase - 0.25).abs() < 1e-6);
        assert!(clock
            .accept_beat(2, 120.0, 1_000_000, 1_250_000, now)
            .is_none());
        assert!(!clock.expire(now + Duration::from_millis(1874)));
        assert!(clock.expire(now + Duration::from_millis(1875)));
        assert!(!clock.expire(now + Duration::from_secs(3)));
    }

    #[test]
    fn master_handoff_drops_old_clock_and_never_follows_our_own_player() {
        let now = Instant::now();
        let mut clock = RemoteClock::default();
        clock.select_master(Some(2), Some(1));
        clock
            .accept_beat(2, 128.0, 1_000_000, 1_000_000, now)
            .unwrap();
        assert!(clock.select_master(Some(3), Some(1)));
        assert!(clock
            .accept_beat(2, 128.0, 1_100_000, 1_100_000, now)
            .is_none());
        assert!(clock
            .accept_beat(3, 128.0, 1_100_000, 1_100_000, now)
            .is_some());
        assert!(clock.select_master(Some(1), Some(1)));
        assert!(clock
            .accept_beat(1, 128.0, 1_200_000, 1_200_000, now)
            .is_none());
    }

    #[test]
    fn invalid_and_stale_packets_cannot_keep_sync_alive() {
        let now = Instant::now();
        let mut clock = RemoteClock::default();
        clock.select_master(Some(2), Some(1));
        assert!(clock
            .accept_beat(2, f64::NAN, 1_000_000, 1_000_000, now)
            .is_none());
        assert!(clock
            .accept_beat(2, 0.0, 1_000_000, 1_000_000, now)
            .is_none());
        assert!(clock
            .accept_beat(2, 128.0, 1_000_000, 3_000_000, now)
            .is_none());
        assert!(clock
            .accept_beat(2, 128.0, 4_000_000, 3_000_000, now)
            .is_none());
        assert!(!clock.connected);
        assert!(clock
            .accept_beat(2, 128.0, 3_000_000, 3_000_000, now)
            .is_some());
    }

    #[test]
    fn slow_master_does_not_expire_between_valid_beats() {
        let now = Instant::now();
        let mut clock = RemoteClock::default();
        clock.select_master(Some(2), Some(1));
        for second in [0, 3, 6] {
            let at = now + Duration::from_secs(second);
            assert!(!clock.expire(at));
            let timestamp = 1_000_000 + second * 1_000_000;
            assert!(clock
                .accept_beat(2, 20.0, timestamp, timestamp, at)
                .is_some());
        }
        assert!(!clock.expire(now + Duration::from_secs(11)));
        assert!(clock.expire(now + Duration::from_secs(12)));
    }

    #[test]
    fn metadata_refresh_detects_analysis_and_cue_changes_without_repeated_sends() {
        let mut deck = deck_snapshot("A");
        deck.track_id = Some("registered-track".into());
        let mut metadata = TrackMetadata {
            track_id: deck.track_id.clone(),
            bpm: deck.original_bpm,
            beats: vec![0.0, 0.5, 1.0],
            hot_cues: deck.hot_cues.clone(),
        };
        let mut cache = MetadataCache::default();
        // The first grid is deliberately sent even with the same visible BPM:
        // reanalysis could have completed between loading and the first poll.
        assert!(cache.needs_refresh(&deck, &metadata));
        cache.mark_sent(&deck, metadata.clone());
        assert!(!cache.needs_refresh(&deck, &metadata));
        metadata.beats = vec![0.1, 0.6, 1.1];
        assert!(cache.needs_refresh(&deck, &metadata));
        cache.mark_sent(&deck, metadata.clone());
        assert!(!cache.needs_refresh(&deck, &metadata));
        metadata.hot_cues[3] = Some(4.0);
        assert!(cache.needs_refresh(&deck, &metadata));
        cache.mark_sent(&deck, metadata.clone());
        // Retry when the authoritative audio snapshot has not adopted the cue.
        assert!(cache.needs_refresh(&deck, &metadata));
        deck.hot_cues = metadata.hot_cues.clone();
        assert!(!cache.needs_refresh(&deck, &metadata));
        metadata.bpm = Some(128.0);
        assert!(cache.needs_refresh(&deck, &metadata));
    }

    #[test]
    fn metadata_refresh_handles_import_after_load_and_retries_after_failed_load() {
        let mut deck = deck_snapshot("B");
        deck.original_bpm = None;
        let metadata = TrackMetadata {
            track_id: Some("newly-imported".into()),
            bpm: None,
            beats: vec![],
            hot_cues: vec![None; 8],
        };
        let mut cache = MetadataCache::default();
        assert!(!cache.needs_refresh(&deck, &TrackMetadata::default()));
        assert!(cache.needs_refresh(&deck, &metadata));
        cache.mark_sent(&deck, metadata.clone());
        assert!(cache.needs_refresh(&deck, &metadata));
        deck.track_id = metadata.track_id.clone();
        assert!(!cache.needs_refresh(&deck, &metadata));
        deck.load_generation += 1;
        deck.loading = true;
        assert!(!cache.needs_refresh(&deck, &metadata));
        deck.loading = false; // Failed decoding leaves the old path loaded.
        assert!(cache.needs_refresh(&deck, &metadata));
        assert_ne!(cache.generation, deck.load_generation);
    }

    #[test]
    fn library_refresh_reads_new_grid_bpm_and_hot_cues_from_registered_path() {
        use conduction_core::{Beat, Key, KeyMode, Track};
        let mut library = conduction_library::Library::in_memory().unwrap();
        let path = Path::new("/music/解析中.wav");
        assert!(read_metadata(&library, path).unwrap().is_none());
        let mut track = Track::placeholder(path.into(), Key::new(8, KeyMode::Minor).unwrap());
        library.insert_track(&track).unwrap();
        let before = read_metadata(&library, path).unwrap().unwrap();
        assert_eq!(before.track_id, Some(track.id.to_string()));
        assert_eq!(before.bpm, None);
        assert!(before.beats.is_empty());
        track.bpm = 125.0;
        library.update_track(&track).unwrap();
        library
            .replace_beatgrid(track.id, &[Beat::new(0.1, true), Beat::new(0.58, false)])
            .unwrap();
        library.set_hot_cue(track.id, 8, 2.5).unwrap();
        let after = read_metadata(&library, path).unwrap().unwrap();
        assert_eq!(after.bpm, Some(125.0));
        assert_eq!(after.beats, vec![0.1, 0.58]);
        assert_eq!(after.hot_cues[7], Some(2.5));
        assert!(!same_metadata(&before, &after));
        library.delete_track(track.id).unwrap();
        assert!(read_metadata(&library, path).unwrap().is_none());
    }
}
