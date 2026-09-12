//! DDJ-FLX4 and DDJ-FLX10 maps transcribed from AlphaTheta's MIDI tables.
//!
//! FLX4: MIDI message list J1, pp. 1-5 (Ver 1.0).
//! FLX10: MIDI message list E1, pp. 1-7.
//! MIDI addresses are the documented bytes; alternate button functions below
//! are Conduction assignments, not a claim to reproduce rekordbox behavior.

use crate::types::*;

const FLX4_SOURCE: &str = "https://downloads.support.alphatheta.com/software_info/dj-controllers/DDJ-FLX4/DDJ-FLX4_MIDI_message_List_J1.pdf";
const FLX10_SOURCE: &str = "https://downloads.support.alphatheta.com/software_info/dj-controllers/DDJ-FLX10/DDJ-FLX10_MIDI_Message_List_E1.pdf";

pub(crate) fn profiles() -> Vec<MidiProfile> {
    vec![profile(false), profile(true)]
}

fn profile(flx10: bool) -> MidiProfile {
    let model = if flx10 { "DDJ-FLX10" } else { "DDJ-FLX4" };
    let mut map = MidiProfile {
        id: model.to_ascii_lowercase(),
        name: model.into(),
        experimental: true,
        sources: vec![if flx10 { FLX10_SOURCE } else { FLX4_SOURCE }.into()],
        warnings: vec![
            "Documented MIDI addresses; hardware validation is still required.".into(),
            "Select HOT CUE mode for the eight cue pads; Shift+pad clears a cue.".into(),
            "Shift+IN halves the loop; Shift+OUT doubles it; Shift+4 BEAT exits it.".into(),
            "CFX/COLOR knobs control Conduction's filter. Proprietary Smart CFX, Smart Fader, STEMS, jog displays, and hardware mixer input routing are not reproduced.".into(),
            "Move FX CH SELECT after connecting to establish the shared FX strip's target. FX parameters apply to Conduction's available effects.".into(),
        ],
        bindings: vec![],
        leds: vec![],
    };
    if flx10 {
        map.warnings.push("Only decks 1 and 2 are mapped. The MASTER, decks 3/4, MIC and SAMPLER FX targets do not control a Conduction deck.".into());
        map.warnings.push("Use HOT CUE PAGE 1. Only the ECHO and REVERB effect selector positions are mapped; other positions leave the software effect unchanged.".into());
    }

    // Both tables use deck channel 1/2 (wire channels 0/1), with
    // performance pads on channels 8/10 and shifted pads on channels 9/11.
    for (channel, deck) in [(0, "A"), (1, "B")] {
        let deck = deck.to_owned();
        button(
            &mut map,
            channel,
            0x0b,
            Control::PlayPause { deck: deck.clone() },
        );
        gate(&mut map, channel, 0x0c, Control::Cue { deck: deck.clone() });
        gate(&mut map, channel, 0x48, Control::Cue { deck: deck.clone() });
        // FLX4 table footnote *3: Sync reports on physical release rather
        // than press. Treat its positive messages as pulses, without needing
        // a separate zero-valued message between consecutive presses.
        map.bindings.push(Binding {
            message: MidiMessage::Note {
                channel,
                number: 0x58,
            },
            control: Control::Sync { deck: deck.clone() },
            mode: if flx10 {
                InputMode::Button
            } else {
                InputMode::Trigger
            },
            pickup: false,
        });
        button(
            &mut map,
            channel,
            0x54,
            Control::HeadphoneCue { deck: deck.clone() },
        );
        for number in [0x36, 0x67] {
            gate(
                &mut map,
                channel,
                number,
                Control::JogTouch { deck: deck.clone() },
            );
        }
        for number in [0x21, 0x22, 0x23, 0x29] {
            relative(
                &mut map,
                channel,
                number,
                Control::Jog { deck: deck.clone() },
                RelativeEncoding::Offset64,
            );
        }
        if flx10 {
            // FLX10 shifted top/rim messages differ from FLX4.
            for number in [0x1f, 0x26] {
                relative(
                    &mut map,
                    channel,
                    number,
                    Control::Jog { deck: deck.clone() },
                    RelativeEncoding::Offset64,
                );
            }
        }
        absolute(
            &mut map,
            channel,
            0x00,
            0x20,
            Control::Tempo { deck: deck.clone() },
        );
        if flx10 {
            absolute(
                &mut map,
                channel,
                0x05,
                0x25,
                Control::Tempo { deck: deck.clone() },
            );
        }
        for (msb, lsb, band) in [
            (0x07, 0x27, "high"),
            (0x0b, 0x2b, "mid"),
            (0x0f, 0x2f, "low"),
        ] {
            absolute(
                &mut map,
                channel,
                msb,
                lsb,
                Control::Eq {
                    deck: deck.clone(),
                    band: band.into(),
                },
            );
        }
        absolute(
            &mut map,
            channel,
            0x13,
            0x33,
            Control::Fader { deck: deck.clone() },
        );
        // COLOR/CFX is on the global mixer channel, not on the deck channel.
        absolute(
            &mut map,
            6,
            0x17 + channel,
            0x37 + channel,
            Control::Filter { deck: deck.clone() },
        );
        for (number, operation) in [
            (0x10, "in"),
            (0x11, "out"),
            (if flx10 { 0x14 } else { 0x4d }, "toggle"),
            (0x50, "exit"),
            (0x4c, "halve"),
            (if flx10 { 0x4d } else { 0x4e }, "double"),
        ] {
            button(
                &mut map,
                channel,
                number,
                Control::Loop {
                    deck: deck.clone(),
                    operation: operation.into(),
                },
            );
        }
        for slot in 0..8 {
            let pad_channel = 7 + channel * 2;
            button(
                &mut map,
                pad_channel,
                slot,
                Control::HotCue {
                    deck: deck.clone(),
                    slot,
                    operation: "trigger_or_set".into(),
                },
            );
            button(
                &mut map,
                pad_channel + 1,
                slot,
                Control::HotCue {
                    deck: deck.clone(),
                    slot,
                    operation: "clear".into(),
                },
            );
            // FLX10 uses color-index velocities 1..127; 127 is a valid
            // documented color. FLX4 uses ordinary 00/7f on/off values.
            led(
                &mut map,
                pad_channel,
                slot,
                LedControl::HotCue {
                    deck: deck.clone(),
                    slot,
                },
            );
            led(
                &mut map,
                pad_channel + 1,
                slot,
                LedControl::HotCue {
                    deck: deck.clone(),
                    slot,
                },
            );
        }
        led(
            &mut map,
            channel,
            0x0b,
            LedControl::Playing { deck: deck.clone() },
        );
        led(
            &mut map,
            channel,
            0x0c,
            LedControl::Cue { deck: deck.clone() },
        );
        led(
            &mut map,
            channel,
            0x58,
            LedControl::Sync { deck: deck.clone() },
        );
        led(&mut map, channel, 0x54, LedControl::HeadphoneCue { deck });
    }

    absolute(&mut map, 6, 0x08, 0x28, Control::Master);
    absolute(&mut map, 6, 0x1f, 0x3f, Control::Crossfader);
    absolute(&mut map, 6, 0x0c, 0x2c, Control::HeadphoneMix);
    absolute(&mut map, 6, 0x0d, 0x2d, Control::HeadphoneVolume);
    for number in [0x40, 0x64] {
        relative(
            &mut map,
            6,
            number,
            Control::Browse,
            RelativeEncoding::TwosComplement,
        );
    }
    button(
        &mut map,
        6,
        0x46,
        Control::LoadSelected { deck: "A".into() },
    );
    button(
        &mut map,
        6,
        0x47,
        Control::LoadSelected { deck: "B".into() },
    );

    if flx10 {
        // Unsupported physical targets must still replace the prior selector,
        // otherwise selecting CH3 would accidentally continue changing CH1.
        for (number, target) in [
            (0x10, "A"),
            (0x11, "B"),
            (0x12, "3"),
            (0x13, "4"),
            (0x14, "master"),
            (0x15, "mic"),
            (0x16, "sampler"),
        ] {
            button(
                &mut map,
                4,
                number,
                Control::FxTarget {
                    deck: target.into(),
                    exclusive: true,
                },
            );
        }
        for (number, effect) in [(0x21, "echo"), (0x24, "reverb")] {
            button(
                &mut map,
                4,
                number,
                Control::Fx {
                    deck: "selected".into(),
                    parameter: format!("select:{effect}"),
                },
            );
        }
    } else {
        // The three-position selector sends one gate for each selected deck.
        // The two other messages in its table are always zero and not bindings.
        gate(
            &mut map,
            4,
            0x10,
            Control::FxTarget {
                deck: "A".into(),
                exclusive: false,
            },
        );
        gate(
            &mut map,
            5,
            0x11,
            Control::FxTarget {
                deck: "B".into(),
                exclusive: false,
            },
        );
        button(
            &mut map,
            4,
            0x63,
            Control::Fx {
                deck: "selected".into(),
                parameter: "select_next".into(),
            },
        );
        button(
            &mut map,
            4,
            0x64,
            Control::Fx {
                deck: "selected".into(),
                parameter: "select_previous".into(),
            },
        );
        button(
            &mut map,
            5,
            0x47,
            Control::Fx {
                deck: "selected".into(),
                parameter: "enabled".into(),
            },
        );
    }
    button(
        &mut map,
        4,
        0x47,
        Control::Fx {
            deck: "selected".into(),
            parameter: "enabled".into(),
        },
    );
    // FLX4's printed channel column says 6 but its wire-status column is B4;
    // use the actual wire status, consistent with FLX10 and both revisions.
    absolute(
        &mut map,
        4,
        0x02,
        0x22,
        Control::Fx {
            deck: "selected".into(),
            parameter: "mix".into(),
        },
    );
    button(
        &mut map,
        4,
        0x4a,
        Control::Fx {
            deck: "selected".into(),
            parameter: "beat_halve".into(),
        },
    );
    button(
        &mut map,
        4,
        0x4b,
        Control::Fx {
            deck: "selected".into(),
            parameter: "beat_double".into(),
        },
    );
    map
}

fn button(map: &mut MidiProfile, channel: u8, number: u8, control: Control) {
    map.bindings.push(Binding {
        message: MidiMessage::Note { channel, number },
        control,
        mode: InputMode::Button,
        pickup: false,
    });
}

fn gate(map: &mut MidiProfile, channel: u8, number: u8, control: Control) {
    map.bindings.push(Binding {
        message: MidiMessage::Note { channel, number },
        control,
        mode: InputMode::Gate,
        pickup: false,
    });
}

fn absolute(map: &mut MidiProfile, channel: u8, msb: u8, lsb: u8, control: Control) {
    // A shared FX strip can target two decks whose wet levels differ; there
    // is no single pickup point. The effect depth knob changes wet directly.
    let pickup = !matches!(control, Control::Fx { .. });
    map.bindings.push(Binding {
        message: MidiMessage::ControlChange {
            channel,
            number: msb,
        },
        control,
        mode: InputMode::Absolute14 { lsb },
        pickup,
    });
}

fn relative(
    map: &mut MidiProfile,
    channel: u8,
    number: u8,
    control: Control,
    encoding: RelativeEncoding,
) {
    map.bindings.push(Binding {
        message: MidiMessage::ControlChange { channel, number },
        control,
        mode: InputMode::Relative {
            encoding,
            step: 1.0,
        },
        pickup: false,
    });
}

fn led(map: &mut MidiProfile, channel: u8, number: u8, control: LedControl) {
    map.leds.push(LedBinding {
        message: MidiMessage::Note { channel, number },
        control,
        on_value: 0x7f,
        off_value: 0,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note_control(profile: &MidiProfile, channel: u8, number: u8) -> Option<&Control> {
        profile
            .bindings
            .iter()
            .find(|binding| binding.message == MidiMessage::Note { channel, number })
            .map(|binding| &binding.control)
    }

    #[test]
    fn model_specific_loop_buttons_do_not_collide() {
        let [flx4, flx10]: [MidiProfile; 2] = profiles().try_into().unwrap();
        for (profile, number, operation) in [
            (&flx4, 0x4d, "toggle"),
            (&flx4, 0x4e, "double"),
            (&flx10, 0x14, "toggle"),
            (&flx10, 0x4d, "double"),
        ] {
            assert_eq!(
                note_control(profile, 0, number),
                Some(&Control::Loop {
                    deck: "A".into(),
                    operation: operation.into()
                })
            );
        }
    }

    #[test]
    fn pads_use_dedicated_channels_and_shift_clears() {
        for profile in profiles() {
            assert_eq!(
                note_control(&profile, 9, 7),
                Some(&Control::HotCue {
                    deck: "B".into(),
                    slot: 7,
                    operation: "trigger_or_set".into()
                })
            );
            assert_eq!(
                note_control(&profile, 10, 7),
                Some(&Control::HotCue {
                    deck: "B".into(),
                    slot: 7,
                    operation: "clear".into()
                })
            );
            // Deck 3/4 controls must not alias deck 1/2.
            assert_eq!(note_control(&profile, 2, 0x0b), None);
            assert_eq!(note_control(&profile, 3, 0x0b), None);
        }
    }

    #[test]
    fn absolute_and_relative_addresses_are_not_ambiguous() {
        use std::collections::HashSet;
        for profile in profiles() {
            let mut addresses = HashSet::new();
            for binding in profile.bindings {
                assert!(
                    addresses.insert(binding.message.clone()),
                    "duplicate primary address"
                );
                if let InputMode::Absolute14 { lsb } = binding.mode {
                    let MidiMessage::ControlChange { channel, .. } = binding.message else {
                        panic!("14-bit controls require CC");
                    };
                    assert_eq!(
                        binding.pickup,
                        !matches!(binding.control, Control::Fx { .. })
                    );
                    assert!(
                        addresses.insert(MidiMessage::ControlChange {
                            channel,
                            number: lsb
                        }),
                        "CC14 LSB overlaps another control"
                    );
                }
            }
        }
    }

    #[test]
    fn recorded_deck_messages_preserve_gates_and_signed_jog_steps() {
        for profile in profiles() {
            let mut decoder = crate::MidiDecoder::new(profile.bindings);
            assert_eq!(
                decoder.process(&[0x90, 0x0b, 0x7f]),
                vec![ControllerAction::PlayPause { deck: "A".into() }]
            );
            assert!(decoder.process(&[0x90, 0x0b, 0x00]).is_empty());
            assert_eq!(
                decoder.process(&[0x91, 0x0c, 0x7f]),
                vec![ControllerAction::Cue {
                    deck: "B".into(),
                    pressed: true
                }]
            );
            assert_eq!(
                decoder.process(&[0x91, 0x0c, 0x00]),
                vec![ControllerAction::Cue {
                    deck: "B".into(),
                    pressed: false
                }]
            );
            assert_eq!(
                decoder.process(&[0xb0, 0x22, 0x3f]),
                vec![ControllerAction::Jog {
                    deck: "A".into(),
                    delta: -1.0
                }]
            );
            assert_eq!(
                decoder.process(&[0xb1, 0x22, 0x42]),
                vec![ControllerAction::Jog {
                    deck: "B".into(),
                    delta: 2.0
                }]
            );
            assert_eq!(
                decoder.process(&[0xb6, 0x40, 0x7f]),
                vec![ControllerAction::Browse { delta: -1 }]
            );
            assert_eq!(
                decoder.process(&[0x99, 0x07, 0x7f]),
                vec![ControllerAction::HotCue {
                    deck: "B".into(),
                    slot: 7,
                    operation: "trigger_or_set".into()
                }]
            );
        }
    }

    #[test]
    fn shared_fx_follows_both_flx4_targets_and_clears_unsupported_flx10_target() {
        let [flx4, flx10]: [MidiProfile; 2] = profiles().try_into().unwrap();
        let mut decoder = crate::MidiDecoder::new(flx4.bindings);
        assert!(decoder.process(&[0x94, 0x47, 0x7f]).is_empty());
        decoder.process(&[0x94, 0x47, 0]);
        decoder.process(&[0x94, 0x10, 0x7f]);
        decoder.process(&[0x95, 0x11, 0x7f]);
        assert_eq!(decoder.process(&[0x94, 0x47, 0x7f]).len(), 2);
        decoder.process(&[0x94, 0x10, 0]);
        assert!(decoder.process(&[0xb4, 0x02, 0x7f]).is_empty());
        assert_eq!(
            decoder.process(&[0xb4, 0x22, 0x7f]),
            vec![ControllerAction::Fx {
                deck: "B".into(),
                parameter: "mix".into(),
                value: 1.0
            }]
        );

        let mut decoder = crate::MidiDecoder::new(flx10.bindings);
        decoder.process(&[0x94, 0x10, 0x7f]);
        assert_eq!(
            decoder.process(&[0x94, 0x24, 0x7f]),
            vec![ControllerAction::Fx {
                deck: "A".into(),
                parameter: "select:reverb".into(),
                value: 1.0
            }]
        );
        decoder.process(&[0x94, 0x12, 0x7f]);
        assert!(decoder.process(&[0x94, 0x47, 0x7f]).is_empty());
    }

    #[test]
    fn tempo_msb_lsb_wait_for_pickup_and_do_not_alias_jog() {
        for profile in profiles() {
            let mut decoder = crate::MidiDecoder::new(profile.bindings);
            let mut state = ControllerState::default();
            state.values.insert("tempo:B".into(), 0.5);
            decoder.update_state(&state);
            assert!(decoder.process(&[0xb1, 0x00, 0]).is_empty());
            assert!(decoder.process(&[0xb1, 0x20, 0]).is_empty());
            assert!(decoder.process(&[0xb1, 0x00, 0x40]).is_empty());
            let actions = decoder.process(&[0xb1, 0x20, 0]);
            assert!(
                matches!(&actions[..], [ControllerAction::Tempo { deck, value }] if deck == "B" && (*value - 0.5).abs() < 0.001)
            );
        }
    }

    #[test]
    fn flx4_sync_accepts_consecutive_release_pulses() {
        let profile = profiles().remove(0);
        let mut decoder = crate::MidiDecoder::new(profile.bindings);
        let expected = vec![ControllerAction::Sync { deck: "A".into() }];
        assert_eq!(decoder.process(&[0x90, 0x58, 0x7f]), expected);
        assert_eq!(decoder.process(&[0x90, 0x58, 0x7f]), expected);
        assert!(decoder.process(&[0x90, 0x58, 0x00]).is_empty());
    }
}
