//! Pure MIDI decoding; no operating system ports are opened by this module.
use crate::types::*;
use std::collections::{HashMap, HashSet, VecDeque};

impl Control {
    pub fn key(&self) -> String {
        match self {
            Self::PlayPause { deck } => format!("play_pause:{deck}"),
            Self::Cue { deck } => format!("cue:{deck}"),
            Self::JogTouch { deck } => format!("jog_touch:{deck}"),
            Self::Jog { deck } => format!("jog:{deck}"),
            Self::Nudge { deck } => format!("nudge:{deck}"),
            Self::Tempo { deck } => format!("tempo:{deck}"),
            Self::Sync { deck } => format!("sync:{deck}"),
            Self::Eq { deck, band } => format!("eq:{deck}:{band}"),
            Self::Filter { deck } => format!("filter:{deck}"),
            Self::Fader { deck } => format!("fader:{deck}"),
            Self::Master => "master".into(),
            Self::Crossfader => "crossfader".into(),
            Self::HeadphoneVolume => "headphone_volume".into(),
            Self::HeadphoneMix => "headphone_mix".into(),
            Self::HeadphoneCue { deck } => format!("headphone_cue:{deck}"),
            Self::HotCue { deck, slot, .. } => format!("hot_cue:{deck}:{slot}"),
            Self::Loop { deck, operation } => format!("loop:{deck}:{operation}"),
            Self::Browse => "browse".into(),
            Self::LoadSelected { deck } => format!("load_selected:{deck}"),
            Self::Fx { deck, parameter } => format!("fx:{deck}:{parameter}"),
            Self::FxTarget { deck, .. } => format!("fx_target:{deck}"),
        }
    }

    pub(crate) fn remap_deck(&mut self, to: &str) {
        let deck = match self {
            Self::PlayPause { deck }
            | Self::Cue { deck }
            | Self::JogTouch { deck }
            | Self::Jog { deck }
            | Self::Nudge { deck }
            | Self::Tempo { deck }
            | Self::Sync { deck }
            | Self::Eq { deck, .. }
            | Self::Filter { deck }
            | Self::Fader { deck }
            | Self::HeadphoneCue { deck }
            | Self::HotCue { deck, .. }
            | Self::Loop { deck, .. }
            | Self::LoadSelected { deck }
            | Self::Fx { deck, .. }
            | Self::FxTarget { deck, .. } => deck,
            _ => return,
        };
        if deck == "A" {
            *deck = to.to_owned();
        }
    }

    fn action(&self, value: f32) -> Option<ControllerAction> {
        Some(match self {
            Self::PlayPause { deck } => ControllerAction::PlayPause { deck: deck.clone() },
            Self::Cue { deck } => ControllerAction::Cue {
                deck: deck.clone(),
                pressed: value != 0.0,
            },
            Self::JogTouch { deck } => ControllerAction::JogTouch {
                deck: deck.clone(),
                touched: value != 0.0,
            },
            Self::Jog { deck } => ControllerAction::Jog {
                deck: deck.clone(),
                delta: value,
            },
            Self::Nudge { deck } => ControllerAction::Nudge {
                deck: deck.clone(),
                amount: value,
            },
            Self::Tempo { deck } => ControllerAction::Tempo {
                deck: deck.clone(),
                value,
            },
            Self::Sync { deck } => ControllerAction::Sync { deck: deck.clone() },
            Self::Eq { deck, band } => ControllerAction::Eq {
                deck: deck.clone(),
                band: band.clone(),
                value,
            },
            Self::Filter { deck } => ControllerAction::Filter {
                deck: deck.clone(),
                value,
            },
            Self::Fader { deck } => ControllerAction::Fader {
                deck: deck.clone(),
                value,
            },
            Self::Master => ControllerAction::Master { value },
            Self::Crossfader => ControllerAction::Crossfader { value },
            Self::HeadphoneVolume => ControllerAction::HeadphoneVolume { value },
            Self::HeadphoneMix => ControllerAction::HeadphoneMix { value },
            Self::HeadphoneCue { deck } => ControllerAction::HeadphoneCue { deck: deck.clone() },
            Self::HotCue {
                deck,
                slot,
                operation,
            } => ControllerAction::HotCue {
                deck: deck.clone(),
                slot: *slot,
                operation: operation.clone(),
            },
            Self::Loop { deck, operation } => ControllerAction::Loop {
                deck: deck.clone(),
                operation: operation.clone(),
            },
            Self::Browse => ControllerAction::Browse {
                delta: value.round() as i32,
            },
            Self::LoadSelected { deck } => ControllerAction::LoadSelected { deck: deck.clone() },
            Self::Fx { deck, parameter } => ControllerAction::Fx {
                deck: deck.clone(),
                parameter: parameter.clone(),
                value,
            },
            Self::FxTarget { .. } => return None,
        })
    }
}

#[derive(Default)]
struct Pickup {
    target: Option<f32>,
    previous: Option<f32>,
    pending: VecDeque<f32>,
    acquired: bool,
}

impl Pickup {
    fn update(&mut self, target: Option<f32>) {
        let target = target.filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0));
        if target != self.target {
            let acknowledged = target.and_then(|value| {
                self.pending
                    .iter()
                    .rposition(|sent| (value - sent).abs() <= 1.0 / 4096.0)
            });
            if let Some(index) = acknowledged {
                // UI snapshots can acknowledge an earlier value while the hand
                // has already moved further. This is not an automation override.
                self.pending.drain(..=index);
                self.acquired = true;
            } else {
                self.acquired = false;
                self.pending.clear();
            }
            self.target = target;
        }
    }

    fn accepts(&mut self, value: f32) -> bool {
        let previous = self.previous.replace(value);
        let Some(target) = self.target else {
            return false;
        };
        if !self.acquired {
            self.acquired = (value - target).abs() <= 2.0 / 127.0
                || previous.is_some_and(|old| (old - target) * (value - target) <= 0.0);
        }
        if self.acquired {
            self.pending.push_back(value);
            if self.pending.len() > 128 {
                self.pending.pop_front();
            }
        }
        self.acquired
    }
}

/// Stateful parser with soft takeover, 14-bit assembly and safe held releases.
pub struct MidiDecoder {
    bindings: Vec<Binding>,
    halves: HashMap<usize, (Option<u8>, Option<u8>)>,
    pressed: HashSet<usize>,
    pickups: HashMap<usize, Pickup>,
    fx_targets: HashSet<String>,
    fx_effect: Option<String>,
}

impl MidiDecoder {
    pub fn new(bindings: Vec<Binding>) -> Self {
        Self {
            bindings,
            halves: HashMap::new(),
            pressed: HashSet::new(),
            pickups: HashMap::new(),
            fx_targets: HashSet::new(),
            fx_effect: None,
        }
    }

    pub fn update_state(&mut self, state: &ControllerState) {
        for (index, binding) in self.bindings.iter().enumerate() {
            if binding.pickup {
                let target = if let Control::Fx { deck, parameter } = &binding.control {
                    if deck == "selected" && self.fx_targets.len() == 1 {
                        self.fx_targets
                            .iter()
                            .next()
                            .and_then(|d| state.values.get(&format!("fx:{d}:{parameter}")).copied())
                    } else {
                        state.values.get(&binding.control.key()).copied()
                    }
                } else {
                    state.values.get(&binding.control.key()).copied()
                };
                self.pickups.entry(index).or_default().update(target);
            }
        }
    }

    pub fn process(&mut self, bytes: &[u8]) -> Vec<ControllerAction> {
        if bytes.len() != 3 || bytes[0] < 0x80 || bytes[1] >= 0x80 || bytes[2] >= 0x80 {
            return vec![];
        }
        let command = bytes[0] & 0xf0;
        let channel = bytes[0] & 0x0f;
        let number = bytes[1];
        let raw = if command == 0x80 { 0 } else { bytes[2] };
        let mut actions = Vec::new();
        for (index, binding) in self.bindings.iter().enumerate() {
            let is_lsb = matches!((&binding.message, &binding.mode),
                (MidiMessage::ControlChange { channel: c, .. }, InputMode::Absolute14 { lsb })
                if command == 0xb0 && channel == *c && number == *lsb);
            let matches = match binding.message {
                MidiMessage::Note {
                    channel: c,
                    number: n,
                } => (command == 0x80 || command == 0x90) && c == channel && n == number,
                MidiMessage::ControlChange {
                    channel: c,
                    number: n,
                } => command == 0xb0 && c == channel && n == number,
                MidiMessage::PitchBend { channel: c } => command == 0xe0 && c == channel,
            };
            if !matches && !is_lsb {
                continue;
            }
            let value = match binding.mode {
                InputMode::Absolute14 { .. } => {
                    let pair = self.halves.entry(index).or_default();
                    if is_lsb {
                        pair.1 = Some(raw);
                    } else {
                        pair.0 = Some(raw);
                    }
                    let (Some(msb), Some(lsb)) = *pair else {
                        continue;
                    };
                    *pair = (None, None);
                    ((u16::from(msb) << 7) | u16::from(lsb)) as f32 / 16383.0
                }
                InputMode::PitchBend14 => {
                    ((u16::from(bytes[2]) << 7) | u16::from(bytes[1])) as f32 / 16383.0
                }
                InputMode::Absolute7 => f32::from(raw) / 127.0,
                InputMode::Relative { encoding, step } => {
                    let delta = match encoding {
                        RelativeEncoding::Offset64 => i16::from(raw) - 64,
                        RelativeEncoding::TwosComplement => {
                            if raw < 64 {
                                i16::from(raw)
                            } else {
                                i16::from(raw) - 128
                            }
                        }
                        RelativeEncoding::SignMagnitude => {
                            if raw < 64 {
                                i16::from(raw)
                            } else {
                                -i16::from(raw & 63)
                            }
                        }
                    };
                    if delta == 0 {
                        continue;
                    }
                    f32::from(delta) * step
                }
                InputMode::Trigger => {
                    if raw == 0 {
                        continue;
                    }
                    1.0
                }
                InputMode::Button | InputMode::Gate => {
                    let was_pressed = self.pressed.contains(&index);
                    if raw == 0 {
                        self.pressed.remove(&index);
                    } else {
                        self.pressed.insert(index);
                    }
                    let selector = matches!(&binding.control, Control::FxTarget { .. })
                        || matches!(&binding.control, Control::Fx { parameter, .. } if parameter.starts_with("select:"));
                    if raw != 0 && was_pressed && !selector {
                        continue;
                    }
                    if matches!(binding.mode, InputMode::Button) && raw == 0 {
                        continue;
                    }
                    if matches!(binding.mode, InputMode::Gate) && raw == 0 && !was_pressed {
                        continue;
                    }
                    if raw == 0 {
                        0.0
                    } else {
                        1.0
                    }
                }
            };
            if let Control::FxTarget { deck, exclusive } = &binding.control {
                if *exclusive {
                    self.fx_targets.clear();
                }
                if value == 0.0 {
                    self.fx_targets.remove(deck);
                } else if deck == "A" || deck == "B" {
                    self.fx_targets.insert(deck.clone());
                    if let Some(effect) = &self.fx_effect {
                        actions.push(ControllerAction::Fx {
                            deck: deck.clone(),
                            parameter: format!("select:{effect}"),
                            value: 1.0,
                        });
                    }
                }
                // A target change must re-arm the shared physical strip.
                for (i, pickup) in &mut self.pickups {
                    if matches!(&self.bindings[*i].control, Control::Fx { deck, .. } if deck == "selected")
                    {
                        pickup.acquired = false;
                    }
                }
                continue;
            }
            if binding.pickup && !self.pickups.entry(index).or_default().accepts(value) {
                continue;
            }
            if let Control::Fx { deck, parameter } = &binding.control {
                if deck == "selected" {
                    let parameter = if let Some(effect) = parameter.strip_prefix("select:") {
                        self.fx_effect = Some(effect.to_owned());
                        parameter.clone()
                    } else if parameter == "select_next" || parameter == "select_previous" {
                        let next = if self.fx_effect.as_deref().unwrap_or("echo") == "echo" {
                            "reverb"
                        } else {
                            "echo"
                        };
                        self.fx_effect = Some(next.into());
                        format!("select:{next}")
                    } else {
                        parameter.clone()
                    };
                    let mut targets: Vec<_> = self.fx_targets.iter().cloned().collect();
                    targets.sort();
                    for deck in targets {
                        actions.push(ControllerAction::Fx {
                            deck,
                            parameter: parameter.clone(),
                            value,
                        });
                    }
                    continue;
                }
            }
            if let Some(action) = binding.control.action(value) {
                actions.push(action);
            }
        }
        actions
    }

    pub fn release_held(&mut self) -> Vec<ControllerAction> {
        let mut indexes: Vec<_> = self.pressed.drain().collect();
        indexes.sort_unstable();
        let actions = indexes
            .into_iter()
            .filter_map(|i| {
                let b = &self.bindings[i];
                if matches!(
                    b.control,
                    Control::Cue { .. } | Control::JogTouch { .. } | Control::Nudge { .. }
                ) {
                    b.control.action(0.0)
                } else {
                    None
                }
            })
            .collect();
        self.halves.clear();
        self.pickups.clear();
        self.fx_targets.clear();
        self.fx_effect = None;
        actions
    }
}

/// Reject ambiguous maps before opening any port.
pub fn validate_bindings(bindings: &[Binding]) -> Result<(), String> {
    let mut occupied = HashSet::new();
    for b in bindings {
        let valid_message = match b.message {
            MidiMessage::Note { channel, number }
            | MidiMessage::ControlChange { channel, number } => channel < 16 && number < 128,
            MidiMessage::PitchBend { channel } => channel < 16,
        };
        if !valid_message {
            return Err("MIDI channels must be 0..15 and values 0..127".into());
        }
        if !occupied.insert(b.message.clone()) {
            return Err(format!("Duplicate MIDI binding: {:?}", b.message));
        }
        if let InputMode::Absolute14 { lsb } = b.mode {
            let MidiMessage::ControlChange { channel, number } = b.message else {
                return Err("14-bit CC requires a control-change message".into());
            };
            if lsb >= 128
                || lsb == number
                || !occupied.insert(MidiMessage::ControlChange {
                    channel,
                    number: lsb,
                })
            {
                return Err("Conflicting 14-bit CC LSB".into());
            }
        }
        if matches!(b.mode, InputMode::PitchBend14)
            != matches!(b.message, MidiMessage::PitchBend { .. })
        {
            return Err("Pitch-bend messages require pitch_bend14 mode".into());
        }
        if matches!(b.mode, InputMode::Relative { step, .. } if !step.is_finite() || step == 0.0) {
            return Err("Relative encoder step must be finite and nonzero".into());
        }
        if matches!(b.mode, InputMode::Relative { .. })
            && !matches!(b.message, MidiMessage::ControlChange { .. })
        {
            return Err("Relative encoders require a control-change message".into());
        }
        if matches!(
            b.control,
            Control::Cue { .. } | Control::JogTouch { .. } | Control::Nudge { .. }
        ) && !matches!(b.mode, InputMode::Gate)
        {
            return Err("Held Cue, jog touch and nudge controls require gate mode".into());
        }
        if b.pickup
            && !matches!(
                b.mode,
                InputMode::Absolute7 | InputMode::Absolute14 { .. } | InputMode::PitchBend14
            )
        {
            return Err("Pickup requires an absolute input".into());
        }
        if matches!(&b.control, Control::HotCue { slot, .. } if *slot > 7) {
            return Err("Hot Cue slot must be 0..7".into());
        }
        match &b.control {
            Control::PlayPause { deck }
            | Control::Cue { deck }
            | Control::JogTouch { deck }
            | Control::Jog { deck }
            | Control::Nudge { deck }
            | Control::Tempo { deck }
            | Control::Sync { deck }
            | Control::Eq { deck, .. }
            | Control::Filter { deck }
            | Control::Fader { deck }
            | Control::HeadphoneCue { deck }
            | Control::HotCue { deck, .. }
            | Control::Loop { deck, .. }
            | Control::LoadSelected { deck }
                if deck != "A" && deck != "B" =>
            {
                return Err("Only deck A or B can be controlled".into())
            }
            Control::Fx { deck, .. } if deck != "A" && deck != "B" && deck != "selected" => {
                return Err("FX deck must be A, B or selected".into())
            }
            Control::Eq { band, .. } if !["low", "mid", "high"].contains(&band.as_str()) => {
                return Err("EQ band must be low, mid or high".into())
            }
            _ => {}
        }
    }
    Ok(())
}

/// Returns LED bytes derived exclusively from confirmed state.
/// The transport retains the previous frame and sends only changed messages.
pub fn led_messages(bindings: &[LedBinding], state: &ControllerState) -> Vec<Vec<u8>> {
    bindings
        .iter()
        .map(|binding| {
            let active = match &binding.control {
                LedControl::Playing { deck } => state.decks.get(deck).is_some_and(|d| d.playing),
                LedControl::Cue { deck } => state.decks.get(deck).is_some_and(|d| d.cue),
                LedControl::Sync { deck } => state.decks.get(deck).is_some_and(|d| d.sync),
                LedControl::HeadphoneCue { deck } => {
                    state.decks.get(deck).is_some_and(|d| d.headphones_cue)
                }
                LedControl::HotCue { deck, slot } => state
                    .decks
                    .get(deck)
                    .is_some_and(|d| d.hot_cues.get(usize::from(*slot)).copied().unwrap_or(false)),
            };
            let value = if active {
                binding.on_value
            } else {
                binding.off_value
            };
            match binding.message {
                MidiMessage::Note { channel, number } => vec![0x90 | channel, number, value],
                MidiMessage::ControlChange { channel, number } => {
                    vec![0xb0 | channel, number, value]
                }
                MidiMessage::PitchBend { channel } => vec![0xe0 | channel, 0, value],
            }
        })
        .collect()
}
