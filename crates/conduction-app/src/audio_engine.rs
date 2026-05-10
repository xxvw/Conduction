//! Audio engine thread host.
//!
//! `rodio::OutputStream` は `!Send` のため、専用スレッドで所有する。
//! UI スレッドからは `AudioHandle` 経由で channel にコマンドを送り、
//! スナップショットを `ArcSwap` で非同期に読み取る。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use arc_swap::ArcSwap;
use conduction_audio::{DeckId, Mixer, OutputDevice, TempoRange};
use conduction_conductor::{
    automation::effective_value as automation_effective, template::DeckSlot as TplDeckSlot,
    AutomationMode, AutomationModeKind, BuiltInTarget, Template, TemplateRunner,
};
use crossbeam::channel::{self, Sender};
use serde::Serialize;
use tracing::{error, info, warn};

/// UI から audio スレッドに送るコマンド。
#[derive(Debug)]
pub enum AudioCommand {
    Load { deck: DeckId, path: PathBuf },
    Play(DeckId),
    Pause(DeckId),
    Stop(DeckId),
    Seek { deck: DeckId, position_sec: f64 },
    SetCrossfader(f32),
    SetChannelVolume { deck: DeckId, volume: f32 },
    SetMasterVolume(f32),
    SetTempoAdjust { deck: DeckId, adjust: f32 },
    SetTempoRange { deck: DeckId, range: TempoRange },
    LoopIn { deck: DeckId, position_sec: f64 },
    LoopOut { deck: DeckId, position_sec: f64 },
    LoopToggle(DeckId),
    LoopClear(DeckId),
    SetEqLow { deck: DeckId, db: f32 },
    SetEqMid { deck: DeckId, db: f32 },
    SetEqHigh { deck: DeckId, db: f32 },
    SetFilter { deck: DeckId, value: f32 },
    SetEcho { deck: DeckId, wet: f32, time_ms: f32, feedback: f32 },
    SetReverb { deck: DeckId, wet: f32, room: f32 },
    SetCueSend { deck: DeckId, value: f32 },
    SetKeyLock { deck: DeckId, on: bool },
    SetPitchOffset { deck: DeckId, semitones: f32 },
    StartTemplate { template: Template, bpm: f32 },
    AbortTemplate,
    OverrideParam { target: BuiltInTarget },
    ResumeParam { target: BuiltInTarget, duration_beats: f64 },
    CommitParam { target: BuiltInTarget },
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
    tx: Sender<AudioCommand>,
    snapshot: Arc<ArcSwap<MixerSnapshot>>,
}

impl AudioHandle {
    pub fn send(&self, cmd: AudioCommand) -> anyhow::Result<()> {
        self.tx
            .send(cmd)
            .map_err(|e| anyhow::anyhow!("audio command channel closed: {e}"))
    }

    pub fn snapshot(&self) -> MixerSnapshot {
        (**self.snapshot.load()).clone()
    }
}

/// audio スレッドを起動してハンドルを返す。
///
/// `main_device_name` で Main 出力デバイスを指定する。`None` ならデフォルト。
/// `cue_device_name` で Cue 出力デバイスを指定する。`None` で Cue 出力なし。
/// 名前指定でオープンに失敗した場合は warning を出してフォールバック（Main は
/// デフォルトデバイス、Cue は無効）。
pub fn spawn(
    main_device_name: Option<String>,
    cue_device_name: Option<String>,
) -> anyhow::Result<AudioHandle> {
    let (tx, rx) = channel::unbounded::<AudioCommand>();
    let initial = empty_snapshot();
    let snapshot = Arc::new(ArcSwap::from_pointee(initial));
    let snapshot_worker = snapshot.clone();

    let (ready_tx, ready_rx) = channel::bounded::<anyhow::Result<()>>(1);

    thread::Builder::new()
        .name("audio-engine".into())
        .spawn(move || {
            let device = match open_main_device(main_device_name.as_deref()) {
                Ok(d) => d,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let cue_device = open_cue_device(cue_device_name.as_deref());
            let mut mixer = match Mixer::new(&device, cue_device.as_ref()) {
                Ok(m) => m,
                Err(e) => {
                    let _ = ready_tx.send(Err(e.into()));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(()));
            info!("audio engine thread started");

            let mut deck_paths: [Option<PathBuf>; 2] = [None, None];
            let mut template_runner: Option<TemplateRunner> = None;
            let mut automation: HashMap<BuiltInTarget, AutomationMode> = HashMap::new();

            loop {
                while let Ok(cmd) = rx.try_recv() {
                    apply_command(
                        &device,
                        cue_device.as_ref(),
                        &mut mixer,
                        &mut deck_paths,
                        &mut template_runner,
                        &mut automation,
                        cmd,
                    );
                }

                // テンプレート進行
                if let Some(runner) = &template_runner {
                    let current_beats = runner.elapsed_beats();
                    for (target, value) in runner.evaluate_now() {
                        let mode =
                            automation.entry(target).or_insert(AutomationMode::Automated);
                        if let Some(eff) = automation_effective(mode, value, current_beats) {
                            apply_template_value(&mut mixer, target, eff);
                        }
                    }
                    if runner.is_done() {
                        info!(name = %runner.template().name, "template completed");
                        template_runner = None;
                        // 終了時は全 mode を Idle に戻す
                        for v in automation.values_mut() {
                            *v = AutomationMode::Idle;
                        }
                    }
                }

                // 各デッキのループ判定。end 到達なら start にシークする。
                if let Err(e) = mixer.deck_a().process_loop() {
                    error!(?e, "loop process failed (deck A)");
                }
                if let Err(e) = mixer.deck_b().process_loop() {
                    error!(?e, "loop process failed (deck B)");
                }
                let snap = build_snapshot(
                    &mut mixer,
                    &deck_paths,
                    template_runner.as_ref(),
                    &automation,
                );
                snapshot_worker.store(Arc::new(snap));

                // 50Hz（20ms）で snapshot 更新。UI の polling は 100ms 程度想定。
                thread::sleep(Duration::from_millis(20));
            }
        })?;

    ready_rx
        .recv()
        .map_err(|_| anyhow::anyhow!("audio engine failed to initialize"))??;

    Ok(AudioHandle { tx, snapshot })
}

fn apply_command(
    device: &OutputDevice,
    cue_device: Option<&OutputDevice>,
    mixer: &mut Mixer,
    paths: &mut [Option<PathBuf>; 2],
    template_runner: &mut Option<TemplateRunner>,
    automation: &mut HashMap<BuiltInTarget, AutomationMode>,
    cmd: AudioCommand,
) {
    match cmd {
        AudioCommand::Load { deck, path } => {
            match mixer.deck(deck).load(device, cue_device, &path) {
                Ok(()) => paths[deck_idx(deck)] = Some(path),
                Err(e) => {
                    error!(?e, ?deck, "load failed");
                    paths[deck_idx(deck)] = None;
                }
            }
        }
        AudioCommand::Play(deck) => mixer.deck(deck).play(),
        AudioCommand::Pause(deck) => mixer.deck(deck).pause(),
        AudioCommand::Stop(deck) => mixer.deck(deck).stop(),
        AudioCommand::Seek { deck, position_sec } => {
            let pos = if position_sec.is_finite() && position_sec >= 0.0 {
                std::time::Duration::from_secs_f64(position_sec)
            } else {
                std::time::Duration::ZERO
            };
            if let Err(e) = mixer.deck(deck).seek(pos) {
                error!(?e, ?deck, "seek failed");
            }
        }
        AudioCommand::SetCrossfader(v) => mixer.set_crossfader(v),
        AudioCommand::SetChannelVolume { deck, volume } => {
            mixer.set_channel_volume(deck, volume);
        }
        AudioCommand::SetMasterVolume(v) => mixer.set_master_volume(v),
        AudioCommand::SetTempoAdjust { deck, adjust } => {
            mixer.deck(deck).set_tempo_adjust(adjust);
        }
        AudioCommand::SetTempoRange { deck, range } => {
            mixer.deck(deck).set_tempo_range(range);
        }
        AudioCommand::LoopIn { deck, position_sec } => {
            mixer.deck(deck).set_loop_in(position_sec);
        }
        AudioCommand::LoopOut { deck, position_sec } => {
            mixer.deck(deck).set_loop_out(position_sec);
        }
        AudioCommand::LoopToggle(deck) => {
            mixer.deck(deck).toggle_loop();
        }
        AudioCommand::LoopClear(deck) => {
            mixer.deck(deck).clear_loop();
        }
        AudioCommand::SetEqLow { deck, db } => {
            mixer.deck(deck).dsp_params().set_eq_low_db(db);
        }
        AudioCommand::SetEqMid { deck, db } => {
            mixer.deck(deck).dsp_params().set_eq_mid_db(db);
        }
        AudioCommand::SetEqHigh { deck, db } => {
            mixer.deck(deck).dsp_params().set_eq_high_db(db);
        }
        AudioCommand::SetFilter { deck, value } => {
            mixer.deck(deck).dsp_params().set_filter(value);
        }
        AudioCommand::SetEcho { deck, wet, time_ms, feedback } => {
            let p = mixer.deck(deck).dsp_params();
            p.set_echo_wet(wet);
            p.set_echo_time_ms(time_ms);
            p.set_echo_feedback(feedback);
        }
        AudioCommand::SetReverb { deck, wet, room } => {
            let p = mixer.deck(deck).dsp_params();
            p.set_reverb_wet(wet);
            p.set_reverb_room(room);
        }
        AudioCommand::SetCueSend { deck, value } => {
            mixer.set_cue_send(deck, value);
        }
        AudioCommand::SetKeyLock { deck, on } => {
            mixer.deck(deck).set_key_lock(on);
        }
        AudioCommand::SetPitchOffset { deck, semitones } => {
            mixer.deck(deck).set_pitch_offset_semitones(semitones);
        }
        AudioCommand::StartTemplate { template, bpm } => {
            info!(
                name = %template.name,
                duration_beats = template.duration_beats,
                bpm,
                "template started"
            );
            // 全 track を Automated 化。既存の Idle/Overridden/Committed を上書き。
            for track in &template.tracks {
                automation.insert(track.target, AutomationMode::Automated);
            }
            *template_runner = Some(TemplateRunner::new(template, bpm));
        }
        AudioCommand::AbortTemplate => {
            if let Some(r) = template_runner.take() {
                info!(name = %r.template().name, "template aborted");
            }
            for v in automation.values_mut() {
                *v = AutomationMode::Idle;
            }
        }
        AudioCommand::OverrideParam { target } => {
            automation.insert(target, AutomationMode::Overridden);
        }
        AudioCommand::ResumeParam {
            target,
            duration_beats,
        } => {
            // resume 時の起点 beat と current value を runner から取り出す。
            let from_value = current_mixer_value(mixer, target);
            let started = template_runner
                .as_ref()
                .map(|r| r.elapsed_beats())
                .unwrap_or(0.0);
            automation.insert(
                target,
                AutomationMode::Resuming {
                    from_value,
                    started_at_beats: started,
                    duration_beats: duration_beats.max(0.25),
                },
            );
        }
        AudioCommand::CommitParam { target } => {
            let fixed_value = current_mixer_value(mixer, target);
            automation.insert(target, AutomationMode::Committed { fixed_value });
        }
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
        BuiltInTarget::DeckEqLow { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().eq_low_db()
        }
        BuiltInTarget::DeckEqMid { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().eq_mid_db()
        }
        BuiltInTarget::DeckEqHigh { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().eq_high_db()
        }
        BuiltInTarget::DeckFilter { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().filter()
        }
        BuiltInTarget::DeckEchoWet { deck } => {
            mixer.deck(to_deck(deck)).dsp_params().echo_wet()
        }
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
    let (head, tail) = key.split_once('.').ok_or_else(|| format!("invalid target key: {key}"))?;
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

fn build_deck_snapshot(
    mixer: &mut Mixer,
    id: DeckId,
    path: Option<&PathBuf>,
) -> DeckSnapshot {
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

fn open_main_device(name: Option<&str>) -> anyhow::Result<OutputDevice> {
    if let Some(n) = name {
        match OutputDevice::open_by_name(n) {
            Ok(d) => return Ok(d),
            Err(e) => {
                warn!(device = %n, error = %e, "failed to open named device, falling back to default");
            }
        }
    }
    Ok(OutputDevice::open_default()?)
}

fn open_cue_device(name: Option<&str>) -> Option<OutputDevice> {
    let n = name?;
    match OutputDevice::open_by_name(n) {
        Ok(d) => {
            info!(device = %d.name(), "cue output device opened");
            Some(d)
        }
        Err(e) => {
            warn!(device = %n, error = %e, "failed to open cue device, disabling Cue output");
            None
        }
    }
}

fn empty_snapshot() -> MixerSnapshot {
    MixerSnapshot {
        crossfader: 0.0,
        master_volume: 1.0,
        deck_a: empty_deck_snapshot("A"),
        deck_b: empty_deck_snapshot("B"),
        template: None,
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
