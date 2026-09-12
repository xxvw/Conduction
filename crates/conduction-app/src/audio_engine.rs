//! Audio engine thread host.
//!
//! CPAL output ownership stays on a dedicated host thread; decoding uses workers.
//! UI スレッドからは `AudioHandle` 経由で channel にコマンドを送り、
//! スナップショットを `ArcSwap` で非同期に読み取る。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use arc_swap::ArcSwap;
use conduction_audio::{AudioOutputConfig, AudioOutputStatus, DeckId, Mixer, TempoRange};
use conduction_conductor::{
    automation::effective_value as automation_effective, template::DeckSlot as TplDeckSlot,
    AutomationMode, AutomationModeKind, BuiltInTarget, Template, TemplateRunner,
};
use crossbeam::channel::{self, Sender};
use serde::{Deserialize, Serialize};
use tracing::warn;

mod host;
use host::{Envelope, Host};

/// UI から audio スレッドに送るコマンド。
#[derive(Debug)]
pub enum AudioCommand {
    Load {
        deck: DeckId,
        path: PathBuf,
    },
    Play(DeckId),
    Pause(DeckId),
    Stop(DeckId),
    Seek {
        deck: DeckId,
        position_sec: f64,
    },
    SetCrossfader(f32),
    SetChannelVolume {
        deck: DeckId,
        volume: f32,
    },
    SetMasterVolume(f32),
    SetTempoAdjust {
        deck: DeckId,
        adjust: f32,
    },
    SetTempoRange {
        deck: DeckId,
        range: TempoRange,
    },
    LoopIn {
        deck: DeckId,
        position_sec: f64,
    },
    LoopOut {
        deck: DeckId,
        position_sec: f64,
    },
    LoopToggle(DeckId),
    LoopClear(DeckId),
    SetEqLow {
        deck: DeckId,
        db: f32,
    },
    SetEqMid {
        deck: DeckId,
        db: f32,
    },
    SetEqHigh {
        deck: DeckId,
        db: f32,
    },
    SetFilter {
        deck: DeckId,
        value: f32,
    },
    SetEcho {
        deck: DeckId,
        wet: f32,
        time_ms: f32,
        feedback: f32,
    },
    SetReverb {
        deck: DeckId,
        wet: f32,
        room: f32,
    },
    SetCueSend {
        deck: DeckId,
        value: f32,
    },
    SetKeyLock {
        deck: DeckId,
        on: bool,
    },
    SetPitchOffset {
        deck: DeckId,
        semitones: f32,
    },
    StartTemplate {
        template: Template,
        bpm: f32,
    },
    AbortTemplate,
    OverrideParam {
        target: BuiltInTarget,
    },
    ResumeParam {
        target: BuiltInTarget,
        duration_beats: f64,
    },
    CommitParam {
        target: BuiltInTarget,
    },
    LoadWithMetadata {
        deck: DeckId,
        path: PathBuf,
        metadata: TrackMetadata,
    },
    SetTrackMetadata {
        deck: DeckId,
        metadata: TrackMetadata,
    },
    RefreshTrackMetadata {
        deck: DeckId,
        path: PathBuf,
        load_generation: u64,
        metadata: TrackMetadata,
    },
    ConfigureAudio(AudioOutputConfig),
    CuePress(DeckId),
    CueRelease(DeckId),
    SetTransportCue {
        deck: DeckId,
        position_sec: f64,
    },
    JogTouch {
        deck: DeckId,
        touched: bool,
    },
    Jog {
        deck: DeckId,
        delta_sec: f64,
    },
    Nudge {
        deck: DeckId,
        value: f32,
    },
    HotCue {
        deck: DeckId,
        slot: u8,
        set: bool,
    },
    SetSync {
        deck: DeckId,
        enabled: bool,
        source: String,
    },
    UpdateLinkClock {
        bpm: f32,
        beat_phase: f64,
        connected: bool,
    },
    SetMasterDeck {
        deck: DeckId,
    },
    SetSyncLatency {
        milliseconds: f64,
    },
    SetHeadphoneMix(f32),
    SetHeadphoneVolume(f32),
    ReleaseControls,
}

/// Analysis metadata is injected by the library service, never queried by audio.
#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct TrackMetadata {
    pub track_id: Option<String>,
    pub bpm: Option<f32>,
    pub beats: Vec<f64>,
    pub hot_cues: Vec<Option<f64>>,
}

/// UI が読む 1 デッキ分のスナップショット。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct DeckSnapshot {
    pub id: &'static str,
    pub state: &'static str,
    pub loaded_path: Option<String>,
    pub channel_volume: f32,
    pub effective_volume: f32,
    pub tempo_range_percent: u8,
    pub tempo_adjust: f32,
    pub playback_speed: f32,
    pub position_sec: f64,
    pub duration_sec: Option<f64>,
    pub loop_start_sec: Option<f64>,
    pub loop_end_sec: Option<f64>,
    pub loop_active: bool,
    pub eq_low_db: f32,
    pub eq_mid_db: f32,
    pub eq_high_db: f32,
    pub filter: f32,
    pub echo_wet: f32,
    pub echo_time_ms: f32,
    pub echo_feedback: f32,
    pub reverb_wet: f32,
    pub reverb_room: f32,
    pub cue_send: f32,
    pub has_cue_output: bool,
    pub key_lock: bool,
    pub pitch_offset_semitones: f32,
    pub track_id: Option<String>,
    pub loading: bool,
    pub load_generation: u64,
    pub load_error: Option<String>,
    pub bpm: Option<f32>,
    pub original_bpm: Option<f32>,
    pub beat_position: Option<f64>,
    pub beat_phase: Option<f64>,
    pub hot_cues: Vec<Option<f64>>,
    pub transport_cue_sec: f64,
    pub cue_pressed: bool,
    pub jog_touched: bool,
    pub nudge: f32,
    pub sync_enabled: bool,
    pub sync_source: Option<String>,
    pub sync_lost: bool,
}

/// Mixer 全体のスナップショット。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct MixerSnapshot {
    pub crossfader: f32,
    pub master_volume: f32,
    pub deck_a: DeckSnapshot,
    pub deck_b: DeckSnapshot,
    /// 実行中テンプレートの状態。`None` なら非実行。
    pub template: Option<TemplateStatus>,
    #[schema(value_type = Object)]
    pub audio: AudioOutputStatus,
    #[schema(value_type = Object)]
    pub audio_config: AudioOutputConfig,
    pub headphone_mix: f32,
    pub headphone_volume: f32,
    pub output_error: Option<String>,
}

/// 実行中テンプレートの UI 向けステータス。
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TemplateStatus {
    pub id: String,
    pub name: String,
    pub progress: f32,
    pub elapsed_beats: f64,
    pub duration_beats: f64,
    pub beats_remaining: f64,
    /// 現在 Overridden / Resuming / Committed のいずれかになっているターゲット数 (UI のカウンタ用)。
    pub override_count: usize,
    /// 各 BuiltInTarget の現在状態 (UI の indicator 用)。
    pub automation_modes: Vec<AutomationModeEntry>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct AutomationModeEntry {
    pub target_key: String,
    pub mode: AutomationModeKind,
}

/// UI 側が保持するハンドル。
#[derive(Clone)]
pub struct AudioHandle {
    tx: Sender<Envelope>,
    snapshot: Arc<ArcSwap<MixerSnapshot>>,
}

impl AudioHandle {
    pub fn send(&self, cmd: AudioCommand) -> anyhow::Result<()> {
        self.tx
            .send(Envelope {
                command: cmd,
                reply: None,
            })
            .map_err(|e| anyhow::anyhow!("audio command channel closed: {e}"))
    }

    pub fn snapshot(&self) -> MixerSnapshot {
        (**self.snapshot.load()).clone()
    }
}

/// Starts the CPAL host. Device errors remain visible and recoverable through configuration.
pub fn spawn(
    main_device_name: Option<String>,
    cue_device_name: Option<String>,
) -> anyhow::Result<AudioHandle> {
    spawn_with_config(AudioOutputConfig {
        device_name: main_device_name,
        cue_device_name,
        ..Default::default()
    })
}

pub fn spawn_with_config(config: AudioOutputConfig) -> anyhow::Result<AudioHandle> {
    let (tx, rx) = channel::unbounded::<Envelope>();
    let snapshot = Arc::new(ArcSwap::from_pointee(empty_snapshot()));
    let snapshot_worker = snapshot.clone();
    let (ready_tx, ready_rx) = channel::bounded(1);
    thread::Builder::new()
        .name("audio-engine".into())
        .spawn(move || {
            let mut host = Host::new(config);
            snapshot_worker.store(Arc::new(host.snapshot()));
            let _ = ready_tx.send(());
            loop {
                match rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(envelope) => host.receive(envelope, &snapshot_worker),
                    Err(channel::RecvTimeoutError::Disconnected) => break,
                    Err(channel::RecvTimeoutError::Timeout) => {}
                }
                while let Ok(envelope) = rx.try_recv() {
                    host.receive(envelope, &snapshot_worker);
                }
                host.tick(&snapshot_worker);
            }
        })?;
    ready_rx
        .recv()
        .map_err(|_| anyhow::anyhow!("audio engine failed to initialize"))?;
    Ok(AudioHandle { tx, snapshot })
}

impl AudioHandle {
    /// Acknowledges the applied command, including actual decode/configuration errors.
    pub fn execute(
        &self,
        command: AudioCommand,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        let (reply, response) = channel::bounded(1);
        let queued = self
            .tx
            .send(Envelope {
                command,
                reply: Some(reply),
            })
            .map_err(|_| anyhow::anyhow!("audio command channel closed"));
        async move {
            queued?;
            tokio::task::spawn_blocking(move || response.recv())
                .await?
                .map_err(|_| anyhow::anyhow!("audio command acknowledgement channel closed"))?
                .map_err(anyhow::Error::msg)
        }
    }

    pub async fn load_track(
        &self,
        deck: DeckId,
        path: PathBuf,
        metadata: TrackMetadata,
    ) -> anyhow::Result<()> {
        self.execute(AudioCommand::LoadWithMetadata {
            deck,
            path,
            metadata,
        })
        .await
    }

    pub async fn configure_audio(&self, config: AudioOutputConfig) -> anyhow::Result<()> {
        self.execute(AudioCommand::ConfigureAudio(config)).await
    }

    pub fn audio_status(&self) -> AudioOutputStatus {
        self.snapshot().audio
    }
}

fn current_mixer_value(mixer: &mut Mixer, target: BuiltInTarget) -> f32 {
    let to_deck = |slot: TplDeckSlot| -> DeckId {
        match slot {
            TplDeckSlot::A => DeckId::A,
            TplDeckSlot::B => DeckId::B,
        }
    };
    match target {
        BuiltInTarget::Crossfader => mixer.crossfader(),
        BuiltInTarget::MasterVolume => mixer.master_volume(),
        BuiltInTarget::DeckVolume { deck } => mixer.deck(to_deck(deck)).channel_volume(),
        BuiltInTarget::DeckEqLow { deck } => mixer.deck(to_deck(deck)).dsp_params().eq_low_db(),
        BuiltInTarget::DeckEqMid { deck } => mixer.deck(to_deck(deck)).dsp_params().eq_mid_db(),
        BuiltInTarget::DeckEqHigh { deck } => mixer.deck(to_deck(deck)).dsp_params().eq_high_db(),
        BuiltInTarget::DeckFilter { deck } => mixer.deck(to_deck(deck)).dsp_params().filter(),
        BuiltInTarget::DeckEchoWet { deck } => mixer.deck(to_deck(deck)).dsp_params().echo_wet(),
        BuiltInTarget::DeckReverbWet { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().reverb_wet()
        }
    }
}

fn apply_template_value(mixer: &mut Mixer, target: BuiltInTarget, value: f32) {
    let to_deck = |slot: TplDeckSlot| -> DeckId {
        match slot {
            TplDeckSlot::A => DeckId::A,
            TplDeckSlot::B => DeckId::B,
        }
    };
    match target {
        BuiltInTarget::Crossfader => mixer.set_crossfader(value),
        BuiltInTarget::MasterVolume => mixer.set_master_volume(value),
        BuiltInTarget::DeckVolume { deck } => {
            mixer.set_channel_volume(to_deck(deck), value);
        }
        BuiltInTarget::DeckEqLow { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().set_eq_low_db(value);
        }
        BuiltInTarget::DeckEqMid { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().set_eq_mid_db(value);
        }
        BuiltInTarget::DeckEqHigh { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().set_eq_high_db(value);
        }
        BuiltInTarget::DeckFilter { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().set_filter(value);
        }
        BuiltInTarget::DeckEchoWet { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().set_echo_wet(value);
        }
        BuiltInTarget::DeckReverbWet { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().set_reverb_wet(value);
        }
    }
}

fn deck_idx(id: DeckId) -> usize {
    match id {
        DeckId::A => 0,
        DeckId::B => 1,
    }
}

fn build_snapshot(
    mixer: &mut Mixer,
    paths: &[Option<PathBuf>; 2],
    template: Option<&TemplateRunner>,
    automation: &HashMap<BuiltInTarget, AutomationMode>,
) -> MixerSnapshot {
    let template_status = template.map(|r| {
        let modes: Vec<AutomationModeEntry> = automation
            .iter()
            .map(|(target, mode)| AutomationModeEntry {
                target_key: target_to_key(*target),
                mode: mode.kind(),
            })
            .collect();
        let override_count = automation
            .values()
            .filter(|m| {
                matches!(
                    m.kind(),
                    AutomationModeKind::Overridden
                        | AutomationModeKind::Resuming
                        | AutomationModeKind::Committed
                )
            })
            .count();
        TemplateStatus {
            id: r.template().id.clone(),
            name: r.template().name.clone(),
            progress: r.progress(),
            elapsed_beats: r.elapsed_beats(),
            duration_beats: r.template().duration_beats,
            beats_remaining: r.beats_remaining(),
            override_count,
            automation_modes: modes,
        }
    });
    MixerSnapshot {
        crossfader: mixer.crossfader(),
        master_volume: mixer.master_volume(),
        deck_a: build_deck_snapshot(mixer, DeckId::A, paths[0].as_ref()),
        deck_b: build_deck_snapshot(mixer, DeckId::B, paths[1].as_ref()),
        template: template_status,
        audio: AudioOutputStatus::default(),
        audio_config: AudioOutputConfig::default(),
        headphone_mix: mixer.headphone_mix(),
        headphone_volume: mixer.headphone_volume(),
        output_error: None,
    }
}

/// `BuiltInTarget` を UI が key として使える短文字列に変換する。
/// UI 側で同じロジック (`lib/automationTargets.ts`) で生成して比較する。
pub fn target_to_key(target: BuiltInTarget) -> String {
    let slot_str = |s: TplDeckSlot| match s {
        TplDeckSlot::A => "A",
        TplDeckSlot::B => "B",
    };
    match target {
        BuiltInTarget::Crossfader => "crossfader".to_string(),
        BuiltInTarget::MasterVolume => "master_volume".to_string(),
        BuiltInTarget::DeckVolume { deck } => format!("deck_volume.{}", slot_str(deck)),
        BuiltInTarget::DeckEqLow { deck } => format!("deck_eq_low.{}", slot_str(deck)),
        BuiltInTarget::DeckEqMid { deck } => format!("deck_eq_mid.{}", slot_str(deck)),
        BuiltInTarget::DeckEqHigh { deck } => format!("deck_eq_high.{}", slot_str(deck)),
        BuiltInTarget::DeckFilter { deck } => format!("deck_filter.{}", slot_str(deck)),
        BuiltInTarget::DeckEchoWet { deck } => format!("deck_echo_wet.{}", slot_str(deck)),
        BuiltInTarget::DeckReverbWet { deck } => format!("deck_reverb_wet.{}", slot_str(deck)),
    }
}

pub fn key_to_target(key: &str) -> Result<BuiltInTarget, String> {
    let slot = |s: &str| -> Result<TplDeckSlot, String> {
        match s {
            "A" => Ok(TplDeckSlot::A),
            "B" => Ok(TplDeckSlot::B),
            other => Err(format!("invalid deck slot: {other}")),
        }
    };
    if key == "crossfader" {
        return Ok(BuiltInTarget::Crossfader);
    }
    if key == "master_volume" {
        return Ok(BuiltInTarget::MasterVolume);
    }
    let (head, tail) = key
        .split_once('.')
        .ok_or_else(|| format!("invalid target key: {key}"))?;
    let deck = slot(tail)?;
    match head {
        "deck_volume" => Ok(BuiltInTarget::DeckVolume { deck }),
        "deck_eq_low" => Ok(BuiltInTarget::DeckEqLow { deck }),
        "deck_eq_mid" => Ok(BuiltInTarget::DeckEqMid { deck }),
        "deck_eq_high" => Ok(BuiltInTarget::DeckEqHigh { deck }),
        "deck_filter" => Ok(BuiltInTarget::DeckFilter { deck }),
        "deck_echo_wet" => Ok(BuiltInTarget::DeckEchoWet { deck }),
        "deck_reverb_wet" => Ok(BuiltInTarget::DeckReverbWet { deck }),
        other => Err(format!("unknown target key prefix: {other}")),
    }
}

fn build_deck_snapshot(mixer: &mut Mixer, id: DeckId, path: Option<&PathBuf>) -> DeckSnapshot {
    let state = deck_state(mixer, id);
    let deck = mixer.deck(id);
    let loop_state = deck.loop_state();
    let dsp = deck.dsp_params();
    DeckSnapshot {
        id: deck_label(id),
        state,
        loaded_path: path.map(|p| p.display().to_string()),
        channel_volume: deck.channel_volume(),
        effective_volume: deck.effective_volume(),
        tempo_range_percent: deck.tempo_range().as_percent(),
        tempo_adjust: deck.tempo_adjust(),
        playback_speed: deck.playback_speed(),
        position_sec: deck.position().as_secs_f64(),
        duration_sec: deck.duration().map(|d| d.as_secs_f64()),
        loop_start_sec: loop_state.start_sec,
        loop_end_sec: loop_state.end_sec,
        loop_active: loop_state.active,
        eq_low_db: dsp.eq_low_db(),
        eq_mid_db: dsp.eq_mid_db(),
        eq_high_db: dsp.eq_high_db(),
        filter: dsp.filter(),
        echo_wet: dsp.echo_wet(),
        echo_time_ms: dsp.echo_time_ms(),
        echo_feedback: dsp.echo_feedback(),
        reverb_wet: dsp.reverb_wet(),
        reverb_room: dsp.reverb_room(),
        cue_send: deck.cue_send(),
        has_cue_output: deck.has_cue_output(),
        key_lock: deck.key_lock(),
        pitch_offset_semitones: deck.pitch_offset_semitones(),
        transport_cue_sec: deck.transport_cue(),
        ..empty_deck_snapshot(deck_label(id))
    }
}

fn deck_state(mixer: &mut Mixer, id: DeckId) -> &'static str {
    let deck = mixer.deck(id);
    if deck.is_finished() {
        "idle"
    } else if deck.is_playing() {
        "play"
    } else if deck.is_paused() {
        "paused"
    } else {
        "loaded"
    }
}

fn deck_label(id: DeckId) -> &'static str {
    match id {
        DeckId::A => "A",
        DeckId::B => "B",
    }
}

fn empty_snapshot() -> MixerSnapshot {
    MixerSnapshot {
        crossfader: 0.0,
        master_volume: 1.0,
        deck_a: empty_deck_snapshot("A"),
        deck_b: empty_deck_snapshot("B"),
        template: None,
        audio: AudioOutputStatus::default(),
        audio_config: AudioOutputConfig::default(),
        headphone_mix: 0.0,
        headphone_volume: 1.0,
        output_error: None,
    }
}

fn empty_deck_snapshot(id: &'static str) -> DeckSnapshot {
    DeckSnapshot {
        id,
        state: "idle",
        loaded_path: None,
        channel_volume: 1.0,
        effective_volume: 1.0,
        tempo_range_percent: 6,
        tempo_adjust: 0.0,
        playback_speed: 1.0,
        position_sec: 0.0,
        duration_sec: None,
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
        has_cue_output: false,
        key_lock: false,
        pitch_offset_semitones: 0.0,
        track_id: None,
        loading: false,
        load_generation: 0,
        load_error: None,
        bpm: None,
        original_bpm: None,
        beat_position: None,
        beat_phase: None,
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

/// 文字列からデッキ ID を解析する（UI との境界で使用）。
pub fn parse_deck(s: &str) -> Result<DeckId, String> {
    match s {
        "A" | "a" => Ok(DeckId::A),
        "B" | "b" => Ok(DeckId::B),
        _ => {
            warn!(?s, "invalid deck id");
            Err(format!("invalid deck id: {s}"))
        }
    }
}

/// 整数パーセンテージから TempoRange を解析する。
pub fn parse_tempo_range(percent: u8) -> Result<TempoRange, String> {
    match percent {
        6 => Ok(TempoRange::Six),
        10 => Ok(TempoRange::Ten),
        16 => Ok(TempoRange::Sixteen),
        other => Err(format!("invalid tempo range: {other}% (expected 6/10/16)")),
    }
}

#[cfg(test)]
mod acknowledgement_tests {
    use super::*;

    #[tokio::test]
    async fn commands_keep_enqueue_order_when_acknowledgements_are_awaited_in_reverse() {
        let (tx, rx) = channel::unbounded();
        let audio = AudioHandle {
            tx,
            snapshot: Arc::new(ArcSwap::from_pointee(empty_snapshot())),
        };
        // These futures deliberately remain unpolled while their requests enter
        // the host queue; decoding can finish independently of MIDI dispatch.
        let first = audio.execute(AudioCommand::Play(DeckId::A));
        let second = audio.execute(AudioCommand::Pause(DeckId::A));
        let first_request = rx
            .try_recv()
            .expect("first command must already be enqueued");
        let second_request = rx
            .try_recv()
            .expect("second command must already be enqueued");
        assert!(matches!(
            first_request.command,
            AudioCommand::Play(DeckId::A)
        ));
        assert!(matches!(
            second_request.command,
            AudioCommand::Pause(DeckId::A)
        ));
        first_request
            .reply
            .unwrap()
            .send(Err("first failed".into()))
            .unwrap();
        second_request.reply.unwrap().send(Ok(())).unwrap();
        second.await.unwrap();
        assert_eq!(first.await.unwrap_err().to_string(), "first failed");
    }
}
