use crate::*;

fn fader_binding(mode: InputMode, pickup: bool) -> Binding {
    Binding {
        message: MidiMessage::ControlChange {
            channel: 0,
            number: 0x13,
        },
        control: Control::Fader { deck: "A".into() },
        mode,
        pickup,
    }
}

fn fader_state(value: f32) -> ControllerState {
    let mut state = ControllerState::default();
    state.values.insert("fader:A".into(), value);
    state
}

#[test]
fn every_documented_profile_is_unambiguous() {
    let profiles = builtin_profiles();
    assert_eq!(profiles.len(), 5);
    for profile in profiles {
        assert!(profile.experimental);
        assert!(!profile.sources.is_empty());
        validate_bindings(&profile.bindings).unwrap_or_else(|e| panic!("{}: {e}", profile.id));
        for led in profile.leds {
            assert!(led.on_value < 128 && led.off_value < 128);
        }
    }
}

#[test]
fn malformed_messages_and_unmapped_channels_are_ignored() {
    let mut decoder = MidiDecoder::new(vec![fader_binding(InputMode::Absolute7, false)]);
    for message in [
        &[][..],
        &[0xb0][..],
        &[0xb0, 0x13][..],
        &[0xb0, 0x13, 0x80][..],
        &[0x40, 0x13, 0x00][..],
        &[0xb2, 0x13, 0x7f][..],
        &[0xf0, 0, 0][..],
    ] {
        assert!(decoder.process(message).is_empty());
    }
    assert_eq!(
        decoder.process(&[0xb0, 0x13, 0x7f]),
        vec![ControllerAction::Fader {
            deck: "A".into(),
            value: 1.0
        }]
    );
}

#[test]
fn fourteen_bit_pairs_do_not_dispatch_half_updated_values() {
    let mut decoder = MidiDecoder::new(vec![fader_binding(
        InputMode::Absolute14 { lsb: 0x33 },
        false,
    )]);
    assert!(decoder.process(&[0xb0, 0x13, 0x7f]).is_empty());
    assert_eq!(
        decoder.process(&[0xb0, 0x33, 0x7f]),
        vec![ControllerAction::Fader {
            deck: "A".into(),
            value: 1.0
        }]
    );
    assert!(decoder.process(&[0xb0, 0x33, 0]).is_empty());
    assert_eq!(
        decoder.process(&[0xb0, 0x13, 0]),
        vec![ControllerAction::Fader {
            deck: "A".into(),
            value: 0.0
        }]
    );
}

#[test]
fn pickup_waits_for_crossing_and_rearms_after_external_changes() {
    let mut decoder = MidiDecoder::new(vec![fader_binding(InputMode::Absolute7, true)]);
    // Unknown state is never guessed from a hardware position.
    assert!(decoder.process(&[0xb0, 0x13, 0]).is_empty());
    decoder.update_state(&fader_state(0.5));
    assert!(decoder.process(&[0xb0, 0x13, 20]).is_empty());
    assert!(!decoder.process(&[0xb0, 0x13, 70]).is_empty());
    decoder.update_state(&fader_state(70.0 / 127.0));
    assert!(!decoder.process(&[0xb0, 0x13, 80]).is_empty());
    decoder.update_state(&fader_state(0.1));
    assert!(decoder.process(&[0xb0, 0x13, 90]).is_empty());
    assert!(!decoder.process(&[0xb0, 0x13, 10]).is_empty());
}

#[test]
fn delayed_acknowledgments_do_not_rearm_a_fader_during_motion() {
    let mut decoder = MidiDecoder::new(vec![fader_binding(InputMode::Absolute7, true)]);
    decoder.update_state(&fader_state(64.0 / 127.0));
    for value in [64, 74, 84] {
        assert!(!decoder.process(&[0xb0, 0x13, value]).is_empty());
    }
    // Backend has applied the first two messages, while the third is in flight.
    decoder.update_state(&fader_state(74.0 / 127.0));
    assert!(!decoder.process(&[0xb0, 0x13, 85]).is_empty());
    decoder.update_state(&fader_state(84.0 / 127.0));
    assert!(!decoder.process(&[0xb0, 0x13, 90]).is_empty());
    // A new automation value still re-arms takeover.
    decoder.update_state(&fader_state(0.1));
    assert!(decoder.process(&[0xb0, 0x13, 100]).is_empty());
}

#[test]
fn hardware_effect_selection_follows_target_changes_without_releasing_selector() {
    let profile = builtin_profiles()
        .into_iter()
        .find(|p| p.id == "ddj-flx10")
        .unwrap();
    let mut decoder = MidiDecoder::new(profile.bindings);
    // Knob selection can arrive before a channel selector has established a target.
    assert!(decoder.process(&[0x94, 0x24, 0x7f]).is_empty());
    for (note, deck) in [(0x10, "A"), (0x11, "B"), (0x10, "A")] {
        assert_eq!(
            decoder.process(&[0x94, note, 0x7f]),
            vec![ControllerAction::Fx {
                deck: deck.into(),
                parameter: "select:reverb".into(),
                value: 1.0,
            }]
        );
    }
    assert_eq!(
        decoder.process(&[0x94, 0x47, 0x7f]),
        vec![ControllerAction::Fx {
            deck: "A".into(),
            parameter: "enabled".into(),
            value: 1.0,
        }]
    );
}

#[test]
fn note_off_velocity_does_not_hold_cue_or_scratching() {
    let mut decoder = MidiDecoder::new(vec![Binding {
        message: MidiMessage::Note {
            channel: 0,
            number: 0x0c,
        },
        control: Control::Cue { deck: "A".into() },
        mode: InputMode::Gate,
        pickup: false,
    }]);
    assert_eq!(
        decoder.process(&[0x90, 0x0c, 127]),
        vec![ControllerAction::Cue {
            deck: "A".into(),
            pressed: true
        }]
    );
    assert_eq!(
        decoder.process(&[0x80, 0x0c, 64]),
        vec![ControllerAction::Cue {
            deck: "A".into(),
            pressed: false
        }]
    );
    assert!(decoder.release_held().is_empty());
    decoder.process(&[0x90, 0x0c, 127]);
    assert_eq!(
        decoder.release_held(),
        vec![ControllerAction::Cue {
            deck: "A".into(),
            pressed: false
        }]
    );
}

#[test]
fn led_feedback_requires_confirmed_backend_state() {
    let profile = builtin_profiles()
        .into_iter()
        .find(|p| p.id == "ddj-flx4")
        .unwrap();
    let mut decoder = MidiDecoder::new(profile.bindings);
    let leds: Vec<_> = profile
        .leds
        .into_iter()
        .filter(|l| l.control == LedControl::Playing { deck: "A".into() })
        .collect();
    let mut state = ControllerState::default();
    assert_eq!(led_messages(&leds, &state), vec![vec![0x90, 0x0b, 0]]);
    decoder.process(&[0x90, 0x0b, 0x7f]);
    assert_eq!(led_messages(&leds, &state), vec![vec![0x90, 0x0b, 0]]);
    state.decks.insert(
        "A".into(),
        MidiDeckState {
            playing: true,
            ..Default::default()
        },
    );
    assert_eq!(led_messages(&leds, &state), vec![vec![0x90, 0x0b, 0x7f]]);
}

#[test]
fn input_maps_reject_overlapping_cc_and_unsupported_decks() {
    let first = fader_binding(InputMode::Absolute14 { lsb: 0x33 }, true);
    let second = Binding {
        message: MidiMessage::ControlChange {
            channel: 0,
            number: 0x33,
        },
        ..fader_binding(InputMode::Absolute7, true)
    };
    assert!(validate_bindings(&[first, second]).is_err());
    let invalid = Binding {
        control: Control::Fader { deck: "C".into() },
        ..fader_binding(InputMode::Absolute7, true)
    };
    assert!(validate_bindings(&[invalid]).is_err());
    let cue_without_release = Binding {
        message: MidiMessage::Note {
            channel: 0,
            number: 0x0c,
        },
        control: Control::Cue { deck: "A".into() },
        mode: InputMode::Button,
        pickup: false,
    };
    assert!(validate_bindings(&[cue_without_release]).is_err());
}

#[test]
fn persistable_profile_overrides_and_action_wire_names() {
    let config = MidiConfig {
        input_port: "DDJ-FLX4".into(),
        output_port: None,
        profile: "ddj-flx4".into(),
        deck: None,
        bindings: vec![fader_binding(InputMode::Absolute7, true)],
    };
    let json = serde_json::to_string(&config).unwrap();
    let decoded: MidiConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.bindings, config.bindings);
    assert_eq!(
        serde_json::to_value(ControllerAction::JogTouch {
            deck: "B".into(),
            touched: true
        })
        .unwrap(),
        serde_json::json!({"action":"jog_touch","deck":"B","touched":true})
    );
}

#[test]
fn worker_lifecycle_and_state_updates_do_not_open_devices() {
    let service = MidiService::new(|_| panic!("No physical ports were opened"));
    for n in 0..100 {
        service.update_state(fader_state(n as f32 / 100.0)).unwrap();
    }
    assert!(service.status().connections.is_empty());
    service.disconnect(None).unwrap();
    drop(service);
}
