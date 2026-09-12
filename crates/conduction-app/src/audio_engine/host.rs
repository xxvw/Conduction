use super::*;
use conduction_audio::{MixingMode, OutputDevice, PcmTrack};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct Envelope {
    pub command: AudioCommand,
    pub reply: Option<Sender<Result<(), String>>>,
}

type Reply = Option<Sender<Result<(), String>>>;
struct DecodeJob {
    deck: DeckId,
    generation: u64,
    path: PathBuf,
    metadata: TrackMetadata,
    reply: Reply,
}
struct DecodeResult {
    job: DecodeJob,
    pcm: Result<Arc<PcmTrack>, String>,
}

#[derive(Default)]
struct DeckState {
    path: Option<PathBuf>,
    pcm: Option<Arc<PcmTrack>>,
    metadata: TrackMetadata,
    generation: u64,
    loading: bool,
    error: Option<String>,
    cue_pressed: bool,
    jog_touched: bool,
    nudge: f32,
    sync_source: Option<String>,
    sync_lost: bool,
    sync_base_speed: Option<f32>,
}

struct LinkClock {
    bpm: f32,
    phase: f64,
    received_at: f64,
    connected: bool,
}

pub(super) struct Host {
    device: Option<OutputDevice>,
    mixer: Option<Mixer>,
    config: AudioOutputConfig,
    error: Option<String>,
    decks: [DeckState; 2],
    generations: [Arc<AtomicU64>; 2],
    decoders: [Sender<DecodeJob>; 2],
    decoded: channel::Receiver<DecodeResult>,
    runner: Option<TemplateRunner>,
    automation: HashMap<BuiltInTarget, AutomationMode>,
    link_clock: Option<LinkClock>,
    sync_latency_sec: f64,
    master_deck: DeckId,
}

impl Host {
    pub fn new(config: AudioOutputConfig) -> Self {
        let mut host = Self::without_output(config.clone());
        if let Err(error) = host.configure(config) {
            host.error = Some(error.to_string());
        }
        host
    }

    fn without_output(config: AudioOutputConfig) -> Self {
        let generations = [Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0))];
        let (complete, decoded) = channel::unbounded();
        let decoders = std::array::from_fn(|index| {
            let (tx, rx) = channel::unbounded::<DecodeJob>();
            let completed = complete.clone();
            let latest = generations[index].clone();
            thread::Builder::new()
                .name(format!("decode-deck-{index}"))
                .spawn(move || {
                    while let Ok(job) = rx.recv() {
                        let pcm = if job.generation == latest.load(Ordering::Acquire) {
                            PcmTrack::decode(&job.path)
                                .map(Arc::new)
                                .map_err(|error| error.to_string())
                        } else {
                            Err("load superseded by a newer track".into())
                        };
                        if completed.send(DecodeResult { job, pcm }).is_err() {
                            break;
                        }
                    }
                })
                .expect("start decode worker");
            tx
        });
        Self {
            device: None,
            mixer: None,
            config: config.clone(),
            error: None,
            decks: std::array::from_fn(|_| DeckState::default()),
            generations,
            decoders,
            decoded,
            runner: None,
            automation: HashMap::new(),
            link_clock: None,
            sync_latency_sec: 0.0,
            master_deck: DeckId::A,
        }
    }

    pub fn receive(&mut self, envelope: Envelope, snapshot: &ArcSwap<MixerSnapshot>) {
        let Envelope { command, reply } = envelope;
        match command {
            AudioCommand::Load { deck, path } => {
                self.load(deck, path, TrackMetadata::default(), reply)
            }
            AudioCommand::LoadWithMetadata {
                deck,
                path,
                metadata,
            } => self.load(deck, path, metadata, reply),
            command => {
                let result = self.apply(command).map_err(|error| error.to_string());
                if let Err(error) = &result {
                    tracing::warn!(%error, "audio command rejected");
                }
                snapshot.store(Arc::new(self.snapshot()));
                respond(reply, result);
            }
        }
    }

    fn load(&mut self, deck: DeckId, path: PathBuf, metadata: TrackMetadata, reply: Reply) {
        if let Err(error) = validate_metadata(&metadata) {
            respond(reply, Err(error));
            return;
        }
        let index = deck_idx(deck);
        let state = &mut self.decks[index];
        state.generation = self.generations[index].fetch_add(1, Ordering::AcqRel) + 1;
        state.loading = true;
        state.error = None;
        let job = DecodeJob {
            deck,
            path,
            metadata,
            generation: state.generation,
            reply,
        };
        if let Err(error) = self.decoders[index].send(job) {
            state.loading = false;
            state.error = Some("decode worker unavailable".into());
            respond(error.0.reply, Err("decode worker unavailable".into()));
        }
    }

    fn finish_load(&mut self, completion: DecodeResult, snapshot: &ArcSwap<MixerSnapshot>) {
        let DecodeResult { job, pcm } = completion;
        let index = deck_idx(job.deck);
        if job.generation != self.decks[index].generation {
            respond(job.reply, Err("load superseded by a newer track".into()));
            return;
        }
        let result = pcm.and_then(|pcm| {
            if let Some(mixer) = &mut self.mixer {
                mixer
                    .deck(job.deck)
                    .load_pcm(pcm.clone())
                    .map_err(|error| error.to_string())?;
                mixer.deck(job.deck).set_sync_speed(None);
            }
            let generation = self.decks[index].generation;
            self.decks[index] = DeckState {
                path: Some(job.path),
                pcm: Some(pcm),
                metadata: job.metadata,
                generation,
                ..Default::default()
            };
            Ok(())
        });
        self.decks[index].loading = false;
        self.decks[index].error = result.as_ref().err().cloned();
        snapshot.store(Arc::new(self.snapshot()));
        respond(job.reply, result);
    }

    fn configure(&mut self, config: AudioOutputConfig) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.runner.is_none(),
            "stop automation before changing audio output"
        );
        anyhow::ensure!(
            !self.decks.iter().any(|deck| deck.loading),
            "wait for track loading before changing audio output"
        );
        if let Some(mixer) = &mut self.mixer {
            anyhow::ensure!(
                !mixer.deck_a().is_playing() && !mixer.deck_b().is_playing(),
                "stop both decks before changing audio output"
            );
        }
        anyhow::ensure!(
            self.decks
                .iter()
                .all(|deck| !deck.cue_pressed && !deck.jog_touched && deck.nudge == 0.0),
            "release Cue, Jog and Nudge before changing audio output"
        );
        let previous = self.snapshot();
        // Open and validate the replacement before touching the current output.
        let device = OutputDevice::open_with_config(&config)?;
        let mut mixer = Mixer::new(&device, None)?;
        for (index, id) in [DeckId::A, DeckId::B].into_iter().enumerate() {
            let old = if index == 0 {
                &previous.deck_a
            } else {
                &previous.deck_b
            };
            if let Some(pcm) = &self.decks[index].pcm {
                mixer.deck(id).load_pcm(pcm.clone())?;
                mixer
                    .deck(id)
                    .seek(Duration::from_secs_f64(old.position_sec))?;
            }
            restore_deck(&mut mixer, id, old);
        }
        mixer.set_crossfader(previous.crossfader);
        mixer.set_master_volume(previous.master_volume);
        mixer.set_headphone_mix(config.headphone_mix);
        mixer.set_headphone_volume(config.headphone_volume);
        self.config = config;
        self.mixer = Some(mixer);
        self.device = Some(device);
        self.error = None;
        // A device change establishes a new output clock epoch.
        self.link_clock = None;
        Ok(())
    }

    fn apply(&mut self, command: AudioCommand) -> anyhow::Result<()> {
        validate_command(&command)?;
        match command {
            AudioCommand::ConfigureAudio(config) => {
                let result = self.configure(config);
                if let Err(error) = &result {
                    self.error = Some(error.to_string());
                }
                return result;
            }
            AudioCommand::SetTrackMetadata { deck, metadata } => {
                validate_metadata(&metadata).map_err(anyhow::Error::msg)?;
                anyhow::ensure!(
                    self.decks[deck_idx(deck)].metadata.track_id == metadata.track_id,
                    "metadata update superseded by a different loaded track"
                );
                self.decks[deck_idx(deck)].metadata = metadata;
                return Ok(());
            }
            AudioCommand::RefreshTrackMetadata {
                deck,
                path,
                load_generation,
                metadata,
            } => {
                validate_metadata(&metadata).map_err(anyhow::Error::msg)?;
                let state = &mut self.decks[deck_idx(deck)];
                anyhow::ensure!(
                    !state.loading
                        && state.generation == load_generation
                        && state.path.as_ref() == Some(&path),
                    "metadata refresh superseded by a track load"
                );
                state.metadata = metadata;
                return Ok(());
            }
            AudioCommand::UpdateLinkClock {
                bpm,
                beat_phase,
                connected,
            } => {
                self.link_clock = Some(LinkClock {
                    bpm,
                    phase: beat_phase.rem_euclid(1.0),
                    received_at: self
                        .device
                        .as_ref()
                        .map_or(0.0, OutputDevice::elapsed_seconds),
                    connected,
                });
                return Ok(());
            }
            AudioCommand::SetMasterDeck { deck } => {
                self.master_deck = deck;
                return Ok(());
            }
            AudioCommand::SetSyncLatency { milliseconds } => {
                anyhow::ensure!(
                    (-500.0..=2000.0).contains(&milliseconds),
                    "sync latency must be -500..2000 ms"
                );
                self.sync_latency_sec = milliseconds / 1000.0;
                return Ok(());
            }
            _ => {}
        }
        let mixer = self.mixer.as_mut().ok_or_else(|| {
            anyhow::anyhow!("audio output is unavailable; select an output device")
        })?;
        if let Some(target) = manual_target(&command) {
            anyhow::ensure!(
                mixer.mode() != MixingMode::External || !is_mixer_target(target),
                "mixer control is bypassed in External mode"
            );
            if self.runner.as_ref().is_some_and(|runner| {
                runner
                    .template()
                    .tracks
                    .iter()
                    .any(|track| track.target == target)
            }) {
                self.automation.insert(target, AutomationMode::Overridden);
            }
        }
        match command {
            AudioCommand::Play(deck) => {
                anyhow::ensure!(self.decks[deck_idx(deck)].pcm.is_some(), "no track loaded");
                anyhow::ensure!(
                    !self
                        .device
                        .as_ref()
                        .is_some_and(|device| device.status().device_lost),
                    "audio output device disconnected"
                );
                mixer.deck(deck).play();
            }
            AudioCommand::Pause(deck) => mixer.deck(deck).pause(),
            AudioCommand::Stop(deck) => {
                release_deck(mixer, deck, &mut self.decks[deck_idx(deck)]);
                mixer.deck(deck).stop();
            }
            AudioCommand::Seek { deck, position_sec } => mixer
                .deck(deck)
                .seek(Duration::from_secs_f64(position_sec))?,
            AudioCommand::SetCrossfader(value) => mixer.set_crossfader(value),
            AudioCommand::SetChannelVolume { deck, volume } => {
                mixer.set_channel_volume(deck, volume)
            }
            AudioCommand::SetMasterVolume(value) => mixer.set_master_volume(value),
            AudioCommand::SetTempoAdjust { deck, adjust } => {
                disable_sync(mixer, deck, &mut self.decks[deck_idx(deck)]);
                mixer.deck(deck).set_tempo_adjust(adjust);
            }
            AudioCommand::SetTempoRange { deck, range } => mixer.deck(deck).set_tempo_range(range),
            AudioCommand::LoopIn { deck, position_sec } => {
                mixer.deck(deck).set_loop_in(position_sec)
            }
            AudioCommand::LoopOut { deck, position_sec } => {
                mixer.deck(deck).set_loop_out(position_sec)
            }
            AudioCommand::LoopToggle(deck) => mixer.deck(deck).toggle_loop(),
            AudioCommand::LoopClear(deck) => mixer.deck(deck).clear_loop(),
            AudioCommand::SetEqLow { deck, db } => mixer.deck(deck).dsp_params().set_eq_low_db(db),
            AudioCommand::SetEqMid { deck, db } => mixer.deck(deck).dsp_params().set_eq_mid_db(db),
            AudioCommand::SetEqHigh { deck, db } => {
                mixer.deck(deck).dsp_params().set_eq_high_db(db)
            }
            AudioCommand::SetFilter { deck, value } => {
                mixer.deck(deck).dsp_params().set_filter(value)
            }
            AudioCommand::SetEcho {
                deck,
                wet,
                time_ms,
                feedback,
            } => {
                let params = mixer.deck(deck).dsp_params();
                params.set_echo_wet(wet);
                params.set_echo_time_ms(time_ms);
                params.set_echo_feedback(feedback);
            }
            AudioCommand::SetReverb { deck, wet, room } => {
                let params = mixer.deck(deck).dsp_params();
                params.set_reverb_wet(wet);
                params.set_reverb_room(room);
            }
            AudioCommand::SetCueSend { deck, value } => mixer.set_cue_send(deck, value),
            AudioCommand::SetKeyLock { deck, on } => mixer.deck(deck).set_key_lock(on),
            AudioCommand::SetPitchOffset { deck, semitones } => {
                mixer.deck(deck).set_pitch_offset_semitones(semitones)
            }
            AudioCommand::StartTemplate { template, bpm } => {
                anyhow::ensure!(
                    template.duration_beats.is_finite() && template.duration_beats > 0.0,
                    "template duration must be positive"
                );
                anyhow::ensure!(
                    mixer.mode() != MixingMode::External
                        || template
                            .tracks
                            .iter()
                            .all(|track| !is_mixer_target(track.target)),
                    "this template requires the software mixer; select Internal mode"
                );
                self.automation.clear();
                for track in &template.tracks {
                    self.automation
                        .insert(track.target, AutomationMode::Automated);
                }
                let mut runner = TemplateRunner::new(template, bpm);
                runner.advance_output_clock(
                    self.device
                        .as_ref()
                        .map_or(0.0, OutputDevice::elapsed_seconds),
                    bpm,
                );
                self.runner = Some(runner);
            }
            AudioCommand::AbortTemplate => {
                self.runner = None;
                self.automation.clear();
            }
            AudioCommand::OverrideParam { target } => {
                self.automation.insert(target, AutomationMode::Overridden);
            }
            AudioCommand::ResumeParam {
                target,
                duration_beats,
            } => {
                let from_value = current_mixer_value(mixer, target);
                let started_at_beats = self
                    .runner
                    .as_ref()
                    .map_or(0.0, TemplateRunner::elapsed_beats);
                self.automation.insert(
                    target,
                    AutomationMode::Resuming {
                        from_value,
                        started_at_beats,
                        duration_beats: duration_beats.max(0.25),
                    },
                );
            }
            AudioCommand::CommitParam { target } => {
                let fixed_value = current_mixer_value(mixer, target);
                self.automation
                    .insert(target, AutomationMode::Committed { fixed_value });
            }
            AudioCommand::CuePress(deck) => {
                anyhow::ensure!(self.decks[deck_idx(deck)].pcm.is_some(), "no track loaded");
                mixer.deck(deck).transport_cue_press();
                self.decks[deck_idx(deck)].cue_pressed = true;
            }
            AudioCommand::CueRelease(deck) => {
                mixer.deck(deck).transport_cue_release();
                self.decks[deck_idx(deck)].cue_pressed = false;
            }
            AudioCommand::SetTransportCue { deck, position_sec } => {
                mixer.deck(deck).set_transport_cue(position_sec)
            }
            AudioCommand::JogTouch { deck, touched } => {
                if touched {
                    disable_sync(mixer, deck, &mut self.decks[deck_idx(deck)]);
                }
                if self.decks[deck_idx(deck)].jog_touched != touched {
                    mixer.deck(deck).set_jog_touch(touched);
                    self.decks[deck_idx(deck)].jog_touched = touched;
                }
            }
            AudioCommand::Jog { deck, delta_sec } => {
                disable_sync(mixer, deck, &mut self.decks[deck_idx(deck)]);
                mixer.deck(deck).jog(delta_sec);
            }
            AudioCommand::Nudge { deck, value } => {
                mixer.deck(deck).set_nudge(value);
                self.decks[deck_idx(deck)].nudge = value.clamp(-0.25, 0.25);
            }
            AudioCommand::HotCue { deck, slot, set } => {
                anyhow::ensure!(slot < 8, "Hot Cue slot must be 0..7");
                anyhow::ensure!(self.decks[deck_idx(deck)].pcm.is_some(), "no track loaded");
                let hot_cues = &mut self.decks[deck_idx(deck)].metadata.hot_cues;
                hot_cues.resize(8, None);
                if set {
                    hot_cues[usize::from(slot)] = Some(mixer.deck(deck).position().as_secs_f64());
                } else {
                    let position = hot_cues[usize::from(slot)]
                        .ok_or_else(|| anyhow::anyhow!("Hot Cue slot is empty"))?;
                    mixer.deck(deck).seek(
                        Duration::try_from_secs_f64(position).map_err(|error| {
                            anyhow::anyhow!("invalid Hot Cue position: {error}")
                        })?,
                    )?;
                }
            }
            AudioCommand::SetSync {
                deck,
                enabled,
                source,
            } => {
                anyhow::ensure!(
                    source == "local" || source == "link",
                    "sync source must be local or link"
                );
                let index = deck_idx(deck);
                if !enabled {
                    disable_sync(mixer, deck, &mut self.decks[index]);
                } else {
                    anyhow::ensure!(
                        has_grid(&self.decks[index].metadata),
                        "analyzed BPM and beat grid required for Sync"
                    );
                    if source == "local" {
                        anyhow::ensure!(
                            has_grid(&self.decks[1 - index].metadata),
                            "other deck needs analyzed BPM and beat grid"
                        );
                        anyhow::ensure!(
                            self.decks[1 - index].sync_source.as_deref() != Some("local"),
                            "other deck already follows this deck"
                        );
                    }
                    self.decks[index].sync_source = Some(source);
                }
            }
            AudioCommand::SetHeadphoneMix(value) => {
                mixer.set_headphone_mix(value);
                self.config.headphone_mix = value.clamp(0.0, 1.0);
            }
            AudioCommand::SetHeadphoneVolume(value) => {
                mixer.set_headphone_volume(value);
                self.config.headphone_volume = value.clamp(0.0, 2.0);
            }
            AudioCommand::ReleaseControls => {
                for deck in [DeckId::A, DeckId::B] {
                    release_deck(mixer, deck, &mut self.decks[deck_idx(deck)]);
                }
            }
            AudioCommand::Load { .. }
            | AudioCommand::LoadWithMetadata { .. }
            | AudioCommand::ConfigureAudio(_)
            | AudioCommand::SetTrackMetadata { .. }
            | AudioCommand::RefreshTrackMetadata { .. }
            | AudioCommand::UpdateLinkClock { .. }
            | AudioCommand::SetMasterDeck { .. }
            | AudioCommand::SetSyncLatency { .. } => unreachable!("handled before mixer access"),
        }
        Ok(())
    }

    pub fn tick(&mut self, snapshot: &ArcSwap<MixerSnapshot>) {
        while let Ok(completion) = self.decoded.try_recv() {
            self.finish_load(completion, snapshot);
        }
        let Some(device) = self.device.as_ref() else {
            snapshot.store(Arc::new(self.snapshot()));
            return;
        };
        let clock = device.elapsed_seconds();
        let status = device.status();
        let mixer = self
            .mixer
            .as_mut()
            .expect("device and mixer installed together");
        if status.device_lost {
            for deck in [DeckId::A, DeckId::B] {
                mixer.deck(deck).pause();
                release_deck(mixer, deck, &mut self.decks[deck_idx(deck)]);
            }
            self.runner = None;
            self.automation.clear();
            self.error = status
                .error
                .or_else(|| Some("audio output device disconnected".into()));
        }
        let positions = [
            mixer.deck_a().position().as_secs_f64(),
            mixer.deck_b().position().as_secs_f64(),
        ];
        let speeds = [
            mixer.deck_a().playback_speed(),
            mixer.deck_b().playback_speed(),
        ];
        let mut master_bpm = None;
        for (index, id) in [DeckId::A, DeckId::B].into_iter().enumerate() {
            let state = &self.decks[index];
            let Some(source) = &state.sync_source else {
                continue;
            };
            let target = if source == "local" {
                let other = &self.decks[1 - index];
                other
                    .metadata
                    .bpm
                    .zip(beat_position(&other.metadata, positions[1 - index]))
                    .map(|(bpm, beat)| (bpm * speeds[1 - index], beat.rem_euclid(1.0)))
            } else {
                self.link_clock
                    .as_ref()
                    .filter(|link| {
                        link.connected
                            && clock >= link.received_at
                            && clock - link.received_at < link_timeout_seconds(link.bpm)
                    })
                    .map(|link| {
                        (
                            link.bpm,
                            (link.phase
                                + (clock - link.received_at + self.sync_latency_sec)
                                    * f64::from(link.bpm)
                                    / 60.0)
                                .rem_euclid(1.0),
                        )
                    })
            };
            if let (Some((target_bpm, phase)), Some(base_bpm), Some(beat)) = (
                target,
                state.metadata.bpm,
                beat_position(&state.metadata, positions[index]),
            ) {
                let difference = phase_error(phase, beat.rem_euclid(1.0));
                let master_moving = source == "link"
                    || mixer
                        .deck(if index == 0 { DeckId::B } else { DeckId::A })
                        .is_playing();
                let correction = if master_moving {
                    (difference * 0.08).clamp(-0.02, 0.02) as f32
                } else {
                    0.0
                };
                let speed = target_bpm / base_bpm * (1.0 + correction);
                mixer.deck(id).set_sync_speed(Some(speed));
                self.decks[index].sync_lost = false;
                self.decks[index].sync_base_speed = Some(target_bpm / base_bpm);
                master_bpm = Some(target_bpm);
            } else {
                // Leave the last valid tempo in place while the source is absent.
                self.decks[index].sync_lost = true;
                if let Some(speed) = self.decks[index].sync_base_speed {
                    mixer.deck(id).set_sync_speed(Some(speed));
                }
            }
        }
        if let Some(runner) = &mut self.runner {
            let bpm = master_bpm
                .or_else(|| {
                    self.decks[deck_idx(self.master_deck)]
                        .metadata
                        .bpm
                        .map(|bpm| bpm * mixer.deck(self.master_deck).playback_speed())
                })
                .unwrap_or(runner.bpm());
            runner.advance_output_clock(clock, bpm);
            let beats = runner.elapsed_beats();
            for (target, value) in runner.evaluate_now() {
                let mode = self
                    .automation
                    .entry(target)
                    .or_insert(AutomationMode::Automated);
                if let Some(value) = automation_effective(mode, value, beats) {
                    apply_template_value(mixer, target, value);
                }
            }
            if runner.is_done() {
                self.runner = None;
                self.automation.clear();
            }
        }
        snapshot.store(Arc::new(self.snapshot()));
    }

    pub fn snapshot(&mut self) -> MixerSnapshot {
        let paths = [self.decks[0].path.clone(), self.decks[1].path.clone()];
        let mut snapshot = self
            .mixer
            .as_mut()
            .map(|mixer| build_snapshot(mixer, &paths, self.runner.as_ref(), &self.automation))
            .unwrap_or_else(empty_snapshot);
        snapshot.audio = self
            .device
            .as_ref()
            .map(OutputDevice::status)
            .unwrap_or_else(|| AudioOutputStatus {
                device_lost: true,
                error: self.error.clone(),
                ..Default::default()
            });
        snapshot.audio_config = self.config.clone();
        snapshot.output_error = self.error.clone().or_else(|| snapshot.audio.error.clone());
        for (index, deck) in [&mut snapshot.deck_a, &mut snapshot.deck_b]
            .into_iter()
            .enumerate()
        {
            let state = &self.decks[index];
            if self.mixer.is_none() && state.pcm.is_some() {
                deck.state = "loaded";
            }
            deck.track_id = state.metadata.track_id.clone();
            deck.loaded_path = state.path.as_ref().map(|path| path.display().to_string());
            deck.duration_sec = state.pcm.as_ref().map(|pcm| pcm.duration().as_secs_f64());
            deck.loading = state.loading;
            deck.load_generation = state.generation;
            deck.load_error = state.error.clone();
            deck.original_bpm = state.metadata.bpm;
            deck.bpm = state.metadata.bpm.map(|bpm| bpm * deck.playback_speed);
            deck.beat_position = beat_position(&state.metadata, deck.position_sec);
            deck.beat_phase = deck.beat_position.map(|beat| beat.rem_euclid(1.0));
            deck.hot_cues = state.metadata.hot_cues.clone();
            deck.hot_cues.resize(8, None);
            deck.cue_pressed = state.cue_pressed;
            deck.jog_touched = state.jog_touched;
            deck.nudge = state.nudge;
            deck.sync_enabled = state.sync_source.is_some();
            deck.sync_source = state.sync_source.clone();
            deck.sync_lost = state.sync_lost;
        }
        snapshot
    }
}

fn respond(reply: Reply, result: Result<(), String>) {
    if let Some(reply) = reply {
        let _ = reply.send(result);
    } else if let Err(error) = result {
        tracing::warn!(%error, "audio operation failed");
    }
}

fn disable_sync(mixer: &mut Mixer, deck: DeckId, state: &mut DeckState) {
    state.sync_source = None;
    state.sync_lost = false;
    state.sync_base_speed = None;
    mixer.deck(deck).set_sync_speed(None);
}

fn release_deck(mixer: &mut Mixer, id: DeckId, state: &mut DeckState) {
    if state.cue_pressed {
        mixer.deck(id).transport_cue_release();
    }
    if state.jog_touched {
        mixer.deck(id).set_jog_touch(false);
    }
    if state.nudge != 0.0 {
        mixer.deck(id).set_nudge(0.0);
    }
    state.cue_pressed = false;
    state.jog_touched = false;
    state.nudge = 0.0;
}

fn restore_deck(mixer: &mut Mixer, id: DeckId, old: &DeckSnapshot) {
    mixer.set_channel_volume(id, old.channel_volume);
    mixer.set_cue_send(id, old.cue_send);
    let deck = mixer.deck(id);
    deck.set_tempo_range(parse_tempo_range(old.tempo_range_percent).unwrap_or_default());
    deck.set_tempo_adjust(old.tempo_adjust);
    deck.set_key_lock(old.key_lock);
    deck.set_pitch_offset_semitones(old.pitch_offset_semitones);
    deck.set_transport_cue(old.transport_cue_sec);
    if let Some(position) = old.loop_start_sec {
        deck.set_loop_in(position);
    }
    if let Some(position) = old.loop_end_sec {
        deck.set_loop_out(position);
    }
    if deck.loop_state().active != old.loop_active {
        deck.toggle_loop();
    }
    let params = deck.dsp_params();
    params.set_eq_low_db(old.eq_low_db);
    params.set_eq_mid_db(old.eq_mid_db);
    params.set_eq_high_db(old.eq_high_db);
    params.set_filter(old.filter);
    params.set_echo_wet(old.echo_wet);
    params.set_echo_time_ms(old.echo_time_ms);
    params.set_echo_feedback(old.echo_feedback);
    params.set_reverb_wet(old.reverb_wet);
    params.set_reverb_room(old.reverb_room);
}

fn manual_target(command: &AudioCommand) -> Option<BuiltInTarget> {
    let slot = |deck: DeckId| {
        if deck == DeckId::A {
            TplDeckSlot::A
        } else {
            TplDeckSlot::B
        }
    };
    Some(match command {
        AudioCommand::SetCrossfader(_) => BuiltInTarget::Crossfader,
        AudioCommand::SetMasterVolume(_) => BuiltInTarget::MasterVolume,
        AudioCommand::SetChannelVolume { deck, .. } => {
            BuiltInTarget::DeckVolume { deck: slot(*deck) }
        }
        AudioCommand::SetEqLow { deck, .. } => BuiltInTarget::DeckEqLow { deck: slot(*deck) },
        AudioCommand::SetEqMid { deck, .. } => BuiltInTarget::DeckEqMid { deck: slot(*deck) },
        AudioCommand::SetEqHigh { deck, .. } => BuiltInTarget::DeckEqHigh { deck: slot(*deck) },
        AudioCommand::SetFilter { deck, .. } => BuiltInTarget::DeckFilter { deck: slot(*deck) },
        AudioCommand::SetEcho { deck, .. } => BuiltInTarget::DeckEchoWet { deck: slot(*deck) },
        AudioCommand::SetReverb { deck, .. } => BuiltInTarget::DeckReverbWet { deck: slot(*deck) },
        _ => return None,
    })
}

fn is_mixer_target(target: BuiltInTarget) -> bool {
    !matches!(
        target,
        BuiltInTarget::DeckEchoWet { .. } | BuiltInTarget::DeckReverbWet { .. }
    )
}

fn has_grid(metadata: &TrackMetadata) -> bool {
    metadata.bpm.is_some_and(|bpm| bpm.is_finite() && bpm > 0.0) && metadata.beats.len() >= 2
}

/// Linear interpolation inside each analyzed interval; extrapolation keeps the first/last interval.
fn beat_position(metadata: &TrackMetadata, position: f64) -> Option<f64> {
    if !has_grid(metadata) || !position.is_finite() {
        return None;
    }
    let next = metadata.beats.partition_point(|beat| *beat <= position);
    let index = next.saturating_sub(1).min(metadata.beats.len() - 2);
    let start = metadata.beats[index];
    let duration = metadata.beats[index + 1] - start;
    (duration > 0.0).then_some(index as f64 + (position - start) / duration)
}

fn link_timeout_seconds(bpm: f32) -> f64 {
    (120.0 / f64::from(bpm)).max(2.0)
}

fn phase_error(target: f64, current: f64) -> f64 {
    (target - current + 0.5).rem_euclid(1.0) - 0.5
}

fn validate_metadata(metadata: &TrackMetadata) -> Result<(), String> {
    if metadata
        .bpm
        .is_some_and(|bpm| !bpm.is_finite() || bpm <= 0.0 || bpm > 1000.0)
    {
        return Err("BPM must be finite and 0..1000".into());
    }
    if metadata
        .beats
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
        || metadata.beats.windows(2).any(|pair| pair[1] <= pair[0])
    {
        return Err("beat grid must contain strictly increasing non-negative times".into());
    }
    if metadata.hot_cues.len() > 8
        || metadata
            .hot_cues
            .iter()
            .flatten()
            .any(|value| !value.is_finite() || *value < 0.0 || *value > 86_400.0)
    {
        return Err("Hot Cues must have at most eight finite, non-negative positions".into());
    }
    Ok(())
}

fn validate_command(command: &AudioCommand) -> anyhow::Result<()> {
    let finite = match command {
        AudioCommand::Seek { position_sec, .. }
        | AudioCommand::LoopIn { position_sec, .. }
        | AudioCommand::LoopOut { position_sec, .. }
        | AudioCommand::SetTransportCue { position_sec, .. } => {
            position_sec.is_finite() && *position_sec >= 0.0 && *position_sec <= 86_400.0
        }
        AudioCommand::SetCrossfader(value)
        | AudioCommand::SetMasterVolume(value)
        | AudioCommand::SetHeadphoneMix(value)
        | AudioCommand::SetHeadphoneVolume(value) => value.is_finite(),
        AudioCommand::SetChannelVolume { volume, .. } => volume.is_finite(),
        AudioCommand::SetTempoAdjust { adjust, .. } => adjust.is_finite(),
        AudioCommand::SetEqLow { db, .. }
        | AudioCommand::SetEqMid { db, .. }
        | AudioCommand::SetEqHigh { db, .. } => db.is_finite(),
        AudioCommand::SetFilter { value, .. }
        | AudioCommand::SetCueSend { value, .. }
        | AudioCommand::Nudge { value, .. } => value.is_finite(),
        AudioCommand::SetEcho {
            wet,
            time_ms,
            feedback,
            ..
        } => wet.is_finite() && time_ms.is_finite() && feedback.is_finite(),
        AudioCommand::SetReverb { wet, room, .. } => wet.is_finite() && room.is_finite(),
        AudioCommand::SetPitchOffset { semitones, .. } => semitones.is_finite(),
        AudioCommand::ResumeParam { duration_beats, .. } => {
            duration_beats.is_finite() && *duration_beats >= 0.0
        }
        AudioCommand::StartTemplate { bpm, .. } => bpm.is_finite() && *bpm > 0.0,
        AudioCommand::UpdateLinkClock {
            bpm,
            beat_phase,
            connected,
        } => !connected || (bpm.is_finite() && *bpm > 0.0 && beat_phase.is_finite()),
        AudioCommand::SetSyncLatency { milliseconds } => milliseconds.is_finite(),
        AudioCommand::Jog { delta_sec, .. } => delta_sec.is_finite() && delta_sec.abs() <= 60.0,
        _ => true,
    };
    anyhow::ensure!(finite, "audio command contains an invalid numeric value");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> TrackMetadata {
        TrackMetadata {
            bpm: Some(120.0),
            beats: vec![0.1, 0.6, 1.1, 1.6],
            ..Default::default()
        }
    }

    #[test]
    fn beat_interpolation_handles_offsets_and_edges() {
        let metadata = grid();
        assert!((beat_position(&metadata, 0.0).unwrap() + 0.2).abs() < 1e-10);
        assert!((beat_position(&metadata, 0.85).unwrap() - 1.5).abs() < 1e-10);
        assert!((beat_position(&metadata, 2.1).unwrap() - 4.0).abs() < 1e-10);
        assert!(beat_position(&TrackMetadata::default(), 0.0).is_none());
    }

    #[test]
    fn slow_link_beats_remain_valid_until_two_beat_periods() {
        assert_eq!(link_timeout_seconds(20.0), 6.0);
        assert_eq!(link_timeout_seconds(120.0), 2.0);
    }

    #[test]
    fn phase_correction_takes_shortest_direction() {
        assert!((phase_error(0.05, 0.95) - 0.1).abs() < 1e-10);
        assert!((phase_error(0.95, 0.05) + 0.1).abs() < 1e-10);
    }

    #[test]
    fn validation_rejects_values_that_break_audio_clock() {
        assert!(validate_command(&AudioCommand::Seek {
            deck: DeckId::A,
            position_sec: f64::NAN
        })
        .is_err());
        assert!(validate_command(&AudioCommand::SetTempoAdjust {
            deck: DeckId::A,
            adjust: f32::INFINITY
        })
        .is_err());
        let mut metadata = grid();
        metadata.beats[1] = metadata.beats[0];
        assert!(validate_metadata(&metadata).is_err());
        let metadata = TrackMetadata {
            hot_cues: vec![Some(1e30)],
            ..Default::default()
        };
        assert!(validate_metadata(&metadata).is_err());
    }

    #[test]
    fn external_allows_effects_and_blocks_mixer_templates() {
        assert!(!is_mixer_target(BuiltInTarget::DeckEchoWet {
            deck: TplDeckSlot::A
        }));
        assert!(Template::long_eq_mix()
            .tracks
            .iter()
            .any(|track| is_mixer_target(track.target)));
        assert_eq!(
            manual_target(&AudioCommand::SetEqLow {
                deck: DeckId::B,
                db: -10.0
            }),
            Some(BuiltInTarget::DeckEqLow {
                deck: TplDeckSlot::B
            })
        );
    }
}

#[cfg(test)]
mod load_tests {
    use super::*;

    fn completion(
        generation: u64,
        path: &str,
        result: Result<Arc<PcmTrack>, String>,
    ) -> DecodeResult {
        DecodeResult {
            job: DecodeJob {
                deck: DeckId::A,
                generation,
                path: PathBuf::from(path),
                metadata: TrackMetadata {
                    track_id: Some(path.into()),
                    ..Default::default()
                },
                reply: None,
            },
            pcm: result,
        }
    }
    fn clip() -> Arc<PcmTrack> {
        Arc::new(PcmTrack {
            sample_rate: 48_000,
            samples: vec![[0.0; 2]; 48].into(),
        })
    }

    #[test]
    fn stale_decode_cannot_replace_latest_track() {
        let mut host = Host::without_output(AudioOutputConfig::default());
        let snapshot = ArcSwap::from_pointee(empty_snapshot());
        host.decks[0].generation = 2;
        host.finish_load(completion(2, "new.wav", Ok(clip())), &snapshot);
        host.finish_load(completion(1, "old.wav", Ok(clip())), &snapshot);
        let actual = host.snapshot();
        assert_eq!(actual.deck_a.track_id.as_deref(), Some("new.wav"));
        assert_eq!(actual.deck_a.loaded_path.as_deref(), Some("new.wav"));
        assert_eq!(actual.deck_a.load_generation, 2);
        assert!(!actual.deck_a.loading);
    }

    #[test]
    fn failed_decode_preserves_playable_clip_and_metadata() {
        let mut host = Host::without_output(AudioOutputConfig::default());
        let snapshot = ArcSwap::from_pointee(empty_snapshot());
        host.decks[0].generation = 1;
        host.finish_load(completion(1, "good.wav", Ok(clip())), &snapshot);
        host.decks[0].generation = 2;
        host.decks[0].loading = true;
        host.finish_load(
            completion(2, "broken.wav", Err("corrupt frame".into())),
            &snapshot,
        );
        let actual = host.snapshot();
        assert_eq!(actual.deck_a.track_id.as_deref(), Some("good.wav"));
        assert_eq!(actual.deck_a.load_error.as_deref(), Some("corrupt frame"));
        assert!(host.decks[0].pcm.is_some());
        assert!(!actual.deck_a.loading);
    }

    #[test]
    fn old_track_metadata_cannot_replace_new_track_identity() {
        let mut host = Host::without_output(AudioOutputConfig::default());
        host.decks[0].metadata.track_id = Some("new-track".into());
        let update = TrackMetadata {
            track_id: Some("old-track".into()),
            ..Default::default()
        };
        assert!(host
            .apply(AudioCommand::SetTrackMetadata {
                deck: DeckId::A,
                metadata: update
            })
            .is_err());
        assert_eq!(
            host.snapshot().deck_a.track_id.as_deref(),
            Some("new-track")
        );
    }

    #[test]
    fn metadata_refresh_can_register_matching_path_but_rejects_stale_generation() {
        let mut host = Host::without_output(AudioOutputConfig::default());
        host.decks[0].path = Some(PathBuf::from("track.wav"));
        host.decks[0].generation = 2;
        let refresh = |generation| AudioCommand::RefreshTrackMetadata {
            deck: DeckId::A,
            path: PathBuf::from("track.wav"),
            load_generation: generation,
            metadata: TrackMetadata {
                track_id: Some("registered-track".into()),
                ..Default::default()
            },
        };
        assert!(host.apply(refresh(1)).is_err());
        assert!(host.apply(refresh(2)).is_ok());
        assert_eq!(
            host.snapshot().deck_a.track_id.as_deref(),
            Some("registered-track")
        );
        host.decks[0].loading = true;
        assert!(host.apply(refresh(2)).is_err());
    }

    #[test]
    fn offline_host_rejects_play_without_opening_hardware() {
        let mut host = Host::without_output(AudioOutputConfig::default());
        assert!(host.apply(AudioCommand::Play(DeckId::A)).is_err());
        assert!(host.snapshot().audio.device_lost);
    }
}
