# Conduction MIDI

Native `midir` input/output for the shared two-deck performance dispatcher.
Profiles contain documented wire addresses and explicit Conduction assignments.
All profiles are experimental pending physical-device and 60-minute soak tests.

## Runtime contract

`MidiService::new(callback)` starts a worker without opening any physical port.
`devices()` enumerates input/output ports; IDs are opaque native endpoint IDs.
`configure(config)` adds or replaces one input connection. Multiple CDJs and a
mixer can remain connected together. An optional output receives supported LEDs.
`disconnect(Some(input_id))` removes one connection; `disconnect(None)` removes all.

The callback receives `ControllerAction` outside the native MIDI callback. Apply
it through the same dispatcher as UI operations, including automation override.
Absolute values are normalized to `[0, 1]`; jog/browse preserve signed encoder
counts. `slot` is a zero-based Hot Cue index. CDJ `deck` can be `A` or `B`.

Send confirmed application state through `update_state(ControllerState)`. It
coalesces updates without an unbounded event queue. LEDs change only when that
state changes. Populate `values` using `Control::key()` for soft takeover, such as
`tempo:A`, `eq:B:low`, `fader:A`, `master`, `crossfader`, `headphone_mix`, and
`headphone_volume`. Unknown takeover values suppress that control until confirmed.
Pending input acknowledgments prevent delayed application snapshots from
incorrectly rearming a moving fader. External changes rearm pickup.

Each configuration stores `input_port`, optional `output_port`, `profile`, optional
CDJ `deck`, and `bindings`. Empty `bindings` selects the built-in input map;
nonempty bindings replace it. Validate and persist overrides in application
settings. Channel numbers are zero based. `Absolute14` names its LSB CC explicitly.
Ambiguous CC overlaps, invalid bytes, and unsupported decks are rejected before
opening a device. Do not parse the native port IDs.

Disconnect and queue overflow release held Cue, jog touch and nudge actions.
Lost devices require explicit reconnection and do not retarget another unit with
the same name. Reconnecting a device resends its supported LED state. Physical
faders must cross the application value after reconnection.

## Profiles and limitations

| ID | Supported assignment | Official source |
| --- | --- | --- |
| `cdj-3000` | One deck, USB MIDI transport, jog, tempo, Sync, loops, Hot Cue, browsing/loading | [MIDI table E108](https://downloads.support.alphatheta.com/software_info/dj-players/CDJ-3000/CDJ3000_MIDI_Message_List_E108.pdf) |
| `cdj-2000nxs2` | One deck, USB MIDI transport, jog, tempo, Sync, loops, Hot Cue, browsing/loading | [Manual, MIDI section](https://downloads.support.alphatheta.com/manuals/CDJ_2000NXS2_DRI1290A_manual.pdf) |
| `djm-a9` | CH1 → A, CH3 → B mixer controls; documented shared FX selection/depth | [MIDI table E10](https://downloads.support.alphatheta.com/midi-mapping/dj-mixers/DJM-A9/DJM-A9_MIDI_Message_List_E_10.pdf) |
| `ddj-flx4` | Two decks, mixer/headphone controls, documented LEDs and shared Echo/Reverb FX | [MIDI table J1](https://downloads.support.alphatheta.com/software_info/dj-controllers/DDJ-FLX4/DDJ-FLX4_MIDI_message_List_J1.pdf) |
| `ddj-flx10` | Decks 1/2, mixer/headphone controls, documented LEDs and shared Echo/Reverb FX | [MIDI table E1](https://downloads.support.alphatheta.com/software_info/dj-controllers/DDJ-FLX10/DDJ-FLX10_MIDI_Message_List_E1.pdf) |

- CDJs use their USB software-control mode. The NXS2 manual states that loading a
  native player track exits software control; LAN/native playback and USB deck
  control are different operating modes for that player.
- CDJ tables document input controls but no LED output map. DJM-A9 does not
  accept MIDI input. These profiles do not infer LED commands.
- The A9 table has conflicting CH2 fader/CUE entries. The default uses the
  unambiguous CH1/CH3 routes. Its ambiguous shared FX ON/OFF and time entries are
  omitted. Choose matching audio output channels in the application.
- FLX10 decks 3/4, STEMS, jog displays and proprietary modes are not mapped. Use
  HOT CUE page 1. FLX4 Smart CFX/Smart Fader are not software features here.
- Move FX CH SELECT after connecting. Unsupported FLX10 FX targets clear the
  selected decks. Echo/Reverb selection is retained when changing target decks.
  Other physical effect-knob positions do not select a supported software effect;
  check the Controller screen's confirmed effect selection.
- Shift+pad clears a Hot Cue. Shift+IN/OUT halves/doubles the loop. These are
  Conduction assignments for documented messages, not rekordbox behavior claims.

## Verification

`cargo test -p conduction-midi` replays official message bytes without opening
physical MIDI ports. Coverage includes model-specific note addresses, 7/14-bit
values, relative encoders, held-action release, FLX4 release pulses, invalid maps,
confirmed-state LEDs, soft takeover, shared FX target changes and port identity.
Actual hardware/firmware responsiveness, USB audio routing and long-running
operation still require the separate device integration test matrix.
