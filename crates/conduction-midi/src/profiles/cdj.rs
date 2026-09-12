//! Pioneer DJ mappings transcribed from the manufacturer's public MIDI tables.
//!
//! These are the standard MIDI modes, not the proprietary USB HID protocols.
//! No MIDI output addresses are inferred from similarly named input controls.

use crate::types::*;

const CDJ_3000_SOURCE: &str = "https://downloads.support.alphatheta.com/software_info/dj-players/CDJ-3000/CDJ3000_MIDI_Message_List_E108.pdf";
const CDJ_NXS2_SOURCE: &str =
    "https://downloads.support.alphatheta.com/manuals/CDJ_2000NXS2_DRI1290A_manual.pdf";
const DJM_A9_SOURCE: &str = "https://downloads.support.alphatheta.com/midi-mapping/dj-mixers/DJM-A9/DJM-A9_MIDI_Message_List_E_10.pdf";

pub(crate) fn profiles() -> Vec<MidiProfile> {
    vec![cdj_3000(), cdj_2000nxs2(), djm_a9()]
}

fn note(number: u8, control: Control, mode: InputMode) -> Binding {
    Binding {
        message: MidiMessage::Note { channel: 0, number },
        control,
        mode,
        pickup: false,
    }
}

fn cc(number: u8, control: Control, mode: InputMode, pickup: bool) -> Binding {
    Binding {
        message: MidiMessage::ControlChange { channel: 0, number },
        control,
        mode,
        pickup,
    }
}

fn cdj_common() -> Vec<Binding> {
    vec![
        note(
            0x00,
            Control::PlayPause { deck: "A".into() },
            InputMode::Button,
        ),
        note(0x01, Control::Cue { deck: "A".into() }, InputMode::Gate),
        cc(
            0x1d,
            Control::Tempo { deck: "A".into() },
            InputMode::Absolute7,
            true,
        ),
        // Both CDJs report signed platter speed around 0x40. The dispatcher
        // interprets the values while the touch gate identifies scratch mode.
        cc(
            0x10,
            Control::Jog { deck: "A".into() },
            InputMode::Relative {
                encoding: RelativeEncoding::Offset64,
                step: 1.0,
            },
            false,
        ),
        cc(
            0x30,
            Control::Jog { deck: "A".into() },
            InputMode::Relative {
                encoding: RelativeEncoding::Offset64,
                step: 1.0,
            },
            false,
        ),
        cc(
            0x4f,
            Control::Browse,
            InputMode::Relative {
                encoding: RelativeEncoding::TwosComplement,
                step: 1.0,
            },
            false,
        ),
    ]
}

fn cdj_transport(bindings: &mut Vec<Binding>, sync: u8, touch: u8, load: u8, loops: [u8; 3]) {
    bindings.extend([
        note(sync, Control::Sync { deck: "A".into() }, InputMode::Button),
        note(
            touch,
            Control::JogTouch { deck: "A".into() },
            InputMode::Gate,
        ),
        note(
            load,
            Control::LoadSelected { deck: "A".into() },
            InputMode::Button,
        ),
    ]);
    for (number, operation) in loops.into_iter().zip(["in", "out", "reloop"]) {
        bindings.push(note(
            number,
            Control::Loop {
                deck: "A".into(),
                operation: operation.into(),
            },
            InputMode::Button,
        ));
    }
}

fn hot_cues(bindings: &mut Vec<Binding>, numbers: [u8; 8]) {
    for (slot, number) in numbers.into_iter().enumerate() {
        bindings.push(note(
            number,
            Control::HotCue {
                deck: "A".into(),
                slot: slot as u8,
                operation: "trigger_or_set".into(),
            },
            InputMode::Button,
        ));
    }
}

fn cdj_warnings() -> Vec<String> {
    vec![
        "Experimental: documented MIDI mapping; hardware/firmware acceptance testing is pending.".into(),
        "Use standard USB MIDI control mode and MIDI channel 1; select deck A or B per player. Other channels require custom bindings.".into(),
        "The public MIDI table documents input only. LED and display feedback are unavailable in this preset; no proprietary HID output is implemented.".into(),
        "Jog sensitivity must be checked on hardware; MIDI reports velocity, not high resolution angular position.".into(),
        "Transport, pitch, jog, sync, browse/load, loop in/out/reloop and eight Hot Cues are mapped. Other controls are not assigned.".into(),
    ]
}

fn cdj_3000() -> MidiProfile {
    let mut bindings = cdj_common();
    cdj_transport(&mut bindings, 0x16, 0x17, 0x11, [0x23, 0x24, 0x25]);
    hot_cues(
        &mut bindings,
        [0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22],
    );
    MidiProfile {
        id: "cdj-3000".into(),
        name: "CDJ-3000 (USB MIDI)".into(),
        experimental: true,
        sources: vec![CDJ_3000_SOURCE.into()],
        warnings: cdj_warnings(),
        bindings,
        leds: Vec::new(),
    }
}

fn cdj_2000nxs2() -> MidiProfile {
    let mut bindings = cdj_common();
    cdj_transport(&mut bindings, 0x1f, 0x20, 0x33, [0x06, 0x07, 0x08]);
    // The NXS2's two hardware banks do NOT use contiguous A-H MIDI notes.
    hot_cues(
        &mut bindings,
        [0x18, 0x19, 0x1a, 0x1b, 0x14, 0x15, 0x16, 0x17],
    );
    let mut warnings = cdj_warnings();
    warnings.push("The CDJ-2000NXS2 leaves USB MIDI control mode when a track is loaded into the player itself (manual page 37).".into());
    MidiProfile {
        id: "cdj-2000nxs2".into(),
        name: "CDJ-2000NXS2 (USB MIDI)".into(),
        experimental: true,
        sources: vec![CDJ_NXS2_SOURCE.into()],
        warnings,
        bindings,
        leds: Vec::new(),
    }
}

fn djm_a9() -> MidiProfile {
    let mut bindings = Vec::new();
    // CH1 -> A, CH3 -> B. CH2 is intentionally excluded: revision E_10 lists
    // its fader as a Note trigger and its CUE B as CC11, conflicting with CH1
    // fader. Do not silently "correct" an ambiguous manufacturer table.
    for (deck, high, mid, low, color, fader, cue) in [
        ("A", 0x02, 0x03, 0x04, 0x05, 0x11, 0x0a),
        ("B", 0x0e, 0x0f, 0x15, 0x16, 0x13, 0x0c),
    ] {
        for (number, band) in [(high, "high"), (mid, "mid"), (low, "low")] {
            bindings.push(cc(
                number,
                Control::Eq {
                    deck: deck.into(),
                    band: band.into(),
                },
                InputMode::Absolute7,
                true,
            ));
        }
        bindings.extend([
            cc(
                color,
                Control::Filter { deck: deck.into() },
                InputMode::Absolute7,
                true,
            ),
            cc(
                fader,
                Control::Fader { deck: deck.into() },
                InputMode::Absolute7,
                true,
            ),
            note(
                cue,
                Control::HeadphoneCue { deck: deck.into() },
                InputMode::Button,
            ),
        ]);
    }
    bindings.extend([
        cc(0x0b, Control::Crossfader, InputMode::Absolute7, true),
        cc(0x18, Control::Master, InputMode::Absolute7, true),
        cc(0x1a, Control::HeadphoneVolume, InputMode::Absolute7, true),
        cc(0x1b, Control::HeadphoneMix, InputMode::Absolute7, true),
    ]);
    // A9's shared Beat FX strip follows its selected physical channel. An
    // unsupported channel removes the previous software target, never silently
    // sending the next depth adjustment to the previously selected deck.
    for number in 0x01..=0x08 {
        let deck = match number {
            0x01 => "A",
            0x03 => "B",
            _ => "unassigned",
        };
        bindings.push(note(
            number,
            Control::FxTarget {
                deck: deck.into(),
                exclusive: true,
            },
            InputMode::Button,
        ));
    }
    bindings.push(cc(
        0x5b,
        Control::Fx {
            deck: "selected".into(),
            parameter: "mix".into(),
        },
        InputMode::Absolute7,
        false,
    ));
    for (number, parameter) in [(0x37, "select:echo"), (0x36, "select:reverb")] {
        bindings.push(cc(
            number,
            Control::Fx {
                deck: "selected".into(),
                parameter: parameter.into(),
            },
            InputMode::Button,
            false,
        ));
    }
    MidiProfile {
        id: "djm-a9".into(),
        name: "DJM-A9 (CH1 / CH3)".into(),
        experimental: true,
        sources: vec![
            DJM_A9_SOURCE.into(),
            "https://support.alphatheta.com/en-us/articles/15997858603545".into(),
        ],
        warnings: vec![
            "Experimental: documented MIDI mapping; hardware/firmware acceptance testing is pending.".into(),
            "CH1 controls deck A and CH3 controls deck B, using MIDI channel 1 and headphones A.".into(),
            "CH2 is excluded because its fader/CUE B entries are ambiguous in the official E_10 table. Use custom bindings after capturing the device's messages.".into(),
            "DJM-A9 does not accept MIDI input; LED output is unavailable (manufacturer FAQ).".into(),
            "Internal mode maps EQ, COLOR as software filter, faders, master and headphones. External mode uses the hardware mixer; software mixer operations are bypassed.".into(),
            "Beat FX channel buttons select A (CH1) or B (CH3); other channels clear the software FX target. ECHO, REVERB and LEVEL/DEPTH control the selected software deck; enable the software effect on screen.".into(),
            "Beat FX ON/OFF is unassigned because the official table reuses CC72 for both ON/OFF and X-PAD touch. TIME and other effect selections are unassigned; they retain the current software effect.".into(),
            "TRIM, microphone, booth and the second headphone bus remain hardware controls; they are not assigned to software parameters.".into(),
        ],
        bindings,
        leds: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control_at(profile: &MidiProfile, message: MidiMessage) -> &Control {
        &profile
            .bindings
            .iter()
            .find(|b| b.message == message)
            .unwrap()
            .control
    }

    #[test]
    fn nxs2_hardware_banks_preserve_hot_cue_slot_identity() {
        let profile = cdj_2000nxs2();
        for (number, slot) in [(0x18, 0), (0x1b, 3), (0x14, 4), (0x17, 7)] {
            assert_eq!(
                control_at(&profile, MidiMessage::Note { channel: 0, number }),
                &Control::HotCue {
                    deck: "A".into(),
                    slot,
                    operation: "trigger_or_set".into(),
                }
            );
        }
    }

    #[test]
    fn model_specific_notes_are_not_shared_accidentally() {
        for (profile, number) in [(cdj_3000(), 0x16), (cdj_2000nxs2(), 0x1f)] {
            assert_eq!(
                control_at(&profile, MidiMessage::Note { channel: 0, number }),
                &Control::Sync { deck: "A".into() }
            );
        }
    }

    #[test]
    fn a9_routes_ch1_and_ch3_without_ambiguous_ch2_messages() {
        let profile = djm_a9();
        for (number, deck) in [(0x11, "A"), (0x13, "B")] {
            assert_eq!(
                control_at(&profile, MidiMessage::ControlChange { channel: 0, number }),
                &Control::Fader { deck: deck.into() }
            );
        }
        assert!(!profile.bindings.iter().any(|b| b.message
            == MidiMessage::Note {
                channel: 0,
                number: 0x12
            }));
    }

    #[test]
    fn input_only_tables_do_not_advertise_led_output() {
        assert!(profiles()
            .iter()
            .all(|p| p.leds.is_empty() && p.experimental));
    }

    #[test]
    fn cdj_capture_sequence_releases_cue_and_scratch_on_disconnect() {
        use crate::MidiDecoder;

        let mut decoder = MidiDecoder::new(cdj_3000().bindings);
        assert_eq!(
            decoder.process(&[0x90, 0x01, 0x7f]),
            vec![ControllerAction::Cue {
                deck: "A".into(),
                pressed: true,
            }]
        );
        assert_eq!(
            decoder.process(&[0x90, 0x17, 0x7f]),
            vec![ControllerAction::JogTouch {
                deck: "A".into(),
                touched: true,
            }]
        );
        assert_eq!(
            decoder.process(&[0xb0, 0x10, 0x3f]),
            vec![ControllerAction::Jog {
                deck: "A".into(),
                delta: -1.0,
            }]
        );
        let releases = decoder.release_held();
        assert!(releases.contains(&ControllerAction::Cue {
            deck: "A".into(),
            pressed: false,
        }));
        assert!(releases.contains(&ControllerAction::JogTouch {
            deck: "A".into(),
            touched: false,
        }));
        assert!(decoder.release_held().is_empty());
    }

    #[test]
    fn browser_encoder_preserves_documented_signed_multi_step_counts() {
        use crate::MidiDecoder;

        let mut decoder = MidiDecoder::new(cdj_2000nxs2().bindings);
        assert_eq!(
            decoder.process(&[0xb0, 0x4f, 0x1e]),
            vec![ControllerAction::Browse { delta: 30 }]
        );
        assert_eq!(
            decoder.process(&[0xb0, 0x4f, 0x62]),
            vec![ControllerAction::Browse { delta: -30 }]
        );
        assert!(decoder.process(&[0xb1, 0x4f, 0x1e]).is_empty());
    }

    #[test]
    fn a9_shared_fx_changes_only_the_selected_supported_channel() {
        use crate::MidiDecoder;

        let mut decoder = MidiDecoder::new(djm_a9().bindings);
        assert!(decoder.process(&[0xb0, 0x5b, 0x7f]).is_empty());
        assert!(decoder.process(&[0x90, 0x03, 0x7f]).is_empty());
        assert!(decoder.process(&[0x90, 0x03, 0x00]).is_empty());
        assert_eq!(
            decoder.process(&[0xb0, 0x5b, 0x7f]),
            vec![ControllerAction::Fx {
                deck: "B".into(),
                parameter: "mix".into(),
                value: 1.0,
            }]
        );
        assert_eq!(
            decoder.process(&[0xb0, 0x37, 0x7f]),
            vec![ControllerAction::Fx {
                deck: "B".into(),
                parameter: "select:echo".into(),
                value: 1.0,
            }]
        );
        // Selecting CH2 must not keep controlling the prior CH3 target.
        assert!(decoder.process(&[0x90, 0x02, 0x7f]).is_empty());
        assert!(decoder.process(&[0xb0, 0x5b, 0x00]).is_empty());
        assert!(decoder.process(&[0xb0, 0x72, 0x7f]).is_empty());
    }
}
