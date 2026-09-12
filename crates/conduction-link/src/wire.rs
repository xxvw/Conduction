//! Bounded Pro DJ Link packet codecs, independently written from the protocol
//! diagrams at <https://djl-analysis.deepsymmetry.org/djl-analysis/>.
//! No code from a third-party protocol implementation is incorporated here.
//!
//! Sources: startup.html, beats.html, vcdj.html, sync.html, media.html and
//! the original packet captures in Deep-Symmetry/dysentery issue #5.

use crate::LocalClock;
use std::net::Ipv4Addr;

const MAGIC: &[u8; 10] = b"Qspt1WmJOL";
const HEADER_LEN: usize = 0x24;
const NORMAL_PITCH: u32 = 0x0010_0000;

#[derive(Clone, Debug)]
pub(crate) struct WireIdentity {
    pub number: u8,
    pub name: String,
    pub ip: Ipv4Addr,
    pub mac: [u8; 6],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiscoveryPacket {
    Keepalive {
        number: u8,
        name: String,
        ip: Ipv4Addr,
        mac: [u8; 6],
        kind: u8,
    },
    Claim {
        number: u8,
    },
    Conflict {
        number: u8,
    },
    Assignment {
        number: u8,
    },
    AssignmentFinished,
    AssignIntent,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BeatPacket {
    pub number: u8,
    /// Effective BPM, including the sending player's pitch adjustment.
    pub bpm: f64,
    /// One-based beat within the bar (1..=4).
    pub beat: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StatusPacket {
    pub number: u8,
    pub name: String,
    pub bpm: Option<f64>,
    pub playing: bool,
    pub synced: bool,
    pub master: bool,
    pub handoff: Option<u8>,
    pub sync_counter: u32,
    /// Absolute one-based beat; zero means unavailable.
    pub beat: u32,
    pub track_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ControlPacket {
    Sync(bool),
    BecomeMaster,
    MasterRequest { number: u8 },
    MasterResponse { number: u8, accepted: bool },
}

fn put_u16(packet: &mut [u8], offset: usize, value: u16) {
    packet[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn put_u32(packet: &mut [u8], offset: usize, value: u32) {
    packet[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn get_u16(packet: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        packet.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn get_u32(packet: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        packet.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn put_name(field: &mut [u8], name: &str) {
    // Device names are an ASCII wire field, not arbitrary UTF-8 text. Keep the
    // field padded and avoid cutting a multi-byte codepoint at byte 20.
    for (out, ch) in field.iter_mut().zip(name.chars()) {
        *out = if ch.is_ascii() && !ch.is_control() {
            ch as u8
        } else {
            b'?'
        };
    }
}

fn put_utf16(field: &mut [u8], text: &str) {
    let mut offset = 0;
    for ch in text.chars() {
        let mut units = [0; 2];
        let encoded = ch.encode_utf16(&mut units);
        // Preserve a complete surrogate pair and one terminating code unit.
        if offset + encoded.len() * 2 + 2 > field.len() {
            break;
        }
        for unit in encoded {
            field[offset..offset + 2].copy_from_slice(&unit.to_be_bytes());
            offset += 2;
        }
    }
}

fn read_name(field: &[u8]) -> String {
    let end = field.iter().position(|b| *b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).into_owned()
}

fn discovery_header(identity: &WireIdentity, kind: u8, subtype: u8, len: usize) -> Vec<u8> {
    let mut packet = vec![0; len];
    packet[..10].copy_from_slice(MAGIC);
    packet[0x0a] = kind;
    put_name(&mut packet[0x0c..0x20], &identity.name);
    packet[0x20] = 1;
    packet[0x21] = subtype;
    put_u16(&mut packet, 0x22, len as u16);
    packet
}

fn status_header(identity: &WireIdentity, kind: u8, subtype: u8, len: usize) -> Vec<u8> {
    let mut packet = vec![0; len];
    packet[..10].copy_from_slice(MAGIC);
    packet[0x0a] = kind;
    put_name(&mut packet[0x0b..0x1f], &identity.name);
    packet[0x1f] = 1;
    packet[0x20] = subtype;
    packet[0x21] = identity.number;
    put_u16(&mut packet, 0x22, (len - HEADER_LEN) as u16);
    packet
}

/// CDJ-3000 compatible startup. Note that the initial trailer is 0x40, while
/// the keepalive trailer is 0x64: the primary diagrams distinguish them.
pub(crate) fn encode_initial(identity: &WireIdentity) -> Vec<u8> {
    let mut packet = discovery_header(identity, 0x0a, 4, 0x26);
    packet[0x24] = 1;
    packet[0x25] = 0x40;
    packet
}

pub(crate) fn encode_claim1(identity: &WireIdentity, count: u8) -> Vec<u8> {
    let mut packet = discovery_header(identity, 0, 3, 0x2c);
    packet[0x24] = count;
    packet[0x25] = 1;
    packet[0x26..0x2c].copy_from_slice(&identity.mac);
    packet
}

pub(crate) fn encode_claim2(identity: &WireIdentity, count: u8) -> Vec<u8> {
    let mut packet = discovery_header(identity, 2, 3, 0x32);
    packet[0x24..0x28].copy_from_slice(&identity.ip.octets());
    packet[0x28..0x2e].copy_from_slice(&identity.mac);
    packet[0x2e] = identity.number;
    packet[0x2f] = count;
    packet[0x30] = 1;
    packet[0x31] = 1; // Auto-assign; abandon the number if it is already in use.
    packet
}

pub(crate) fn encode_claim_final(identity: &WireIdentity, count: u8) -> Vec<u8> {
    // The prose startup table says 0x2a, but the original LinkInfo capture
    // contains a 0x26-byte final claim. Its length field also says 0x26.
    let mut packet = discovery_header(identity, 4, 3, 0x26);
    packet[0x24] = identity.number;
    packet[0x25] = count;
    packet
}

pub(crate) fn encode_assignment_finished(identity: &WireIdentity) -> Vec<u8> {
    let mut packet = discovery_header(identity, 5, 2, 0x26);
    packet[0x24] = identity.number;
    packet[0x25] = 1;
    packet
}

pub(crate) fn encode_keepalive(
    identity: &WireIdentity,
    peer_count: u8,
    was_first: bool,
    library: bool,
) -> Vec<u8> {
    if library {
        // Export-mode rekordbox uses a recurring type-2 announcement, not
        // the player's type-6 keepalive or the lighting-mode source format.
        // Protocol facts: vynull's public MarshalRekordboxKeepAlive interface.
        let mut packet = discovery_header(identity, 2, 3, 0x32);
        packet[0x24..0x28].copy_from_slice(&identity.ip.octets());
        packet[0x28..0x2e].copy_from_slice(&identity.mac);
        packet[0x2e] = identity.number;
        packet[0x2f] = 6;
        packet[0x30] = 4; // rekordbox source class, distinct from a CDJ claim.
        packet[0x31] = 1;
        return packet;
    }
    let mut packet = discovery_header(identity, 6, 2, 0x36);
    packet[0x24] = identity.number;
    packet[0x25] = if was_first { 2 } else { 1 };
    packet[0x26..0x2c].copy_from_slice(&identity.mac);
    packet[0x2c..0x30].copy_from_slice(&identity.ip.octets());
    packet[0x30] = peer_count.max(1);
    packet[0x34] = 1;
    packet[0x35] = 0x64;
    packet
}

fn bpm_word(bpm: f64) -> u16 {
    if bpm.is_finite() && bpm > 0.0 && bpm < 655.35 {
        (bpm * 100.0).round().clamp(1.0, 65534.0) as u16
    } else {
        0xffff
    }
}

fn beat_in_bar(beat: u32) -> u8 {
    if beat == 0 || beat == u32::MAX {
        0
    } else {
        ((beat - 1) % 4 + 1) as u8
    }
}

pub(crate) fn encode_status(
    identity: &WireIdentity,
    clock: &LocalClock,
    master: bool,
    handoff: Option<u8>,
    sync_counter: u32,
    packet_counter: u32,
) -> Vec<u8> {
    let mut packet = status_header(identity, 0x0a, 3, 0xd4);
    let loaded = clock.track_id.is_some_and(|id| id != 0);
    let grid = loaded && clock.grid_available && bpm_word(clock.bpm) != 0xffff;
    let playing = loaded && clock.playing;
    packet[0x24] = identity.number;
    packet[0x26] = 1;
    packet[0x27] = u8::from(playing);
    if let Some(track_id) = clock.track_id.filter(|id| *id != 0) {
        packet[0x28] = 17; // The accompanying library source owns this ID.
        packet[0x29] = 4; // rekordbox collection slot.
        packet[0x2a] = if grid { 1 } else { 2 };
        put_u32(&mut packet, 0x2c, track_id);
        packet[0x37] = 4; // Track menu.
    }
    packet[0x68] = 1;
    packet[0x6f] = 4; // No USB or SD media inserted in the virtual player.
    packet[0x73] = 4;
    packet[0x75] = u8::from(loaded);
    packet[0x78] = 1;
    packet[0x7b] = if playing {
        3
    } else if loaded {
        5
    } else {
        0
    };
    packet[0x7c..0x80].copy_from_slice(b"1.00");
    put_u32(&mut packet, 0x84, sync_counter);
    packet[0x89] = 0x84
        | if playing { 0x40 } else { 0 }
        | if master { 0x20 } else { 0 }
        | if grid && clock.synced { 0x10 } else { 0 };
    packet[0x8a] = 0xff;
    packet[0x8b] = if playing { 0x7a } else { 0x7e };
    // LocalClock reports effective BPM, so all nominal pitch values are 0%.
    put_u32(&mut packet, 0x8c, NORMAL_PITCH);
    put_u16(
        &mut packet,
        0x90,
        if grid {
            0x8000
        } else if loaded {
            0
        } else {
            0x7fff
        },
    );
    put_u16(
        &mut packet,
        0x92,
        if loaded { bpm_word(clock.bpm) } else { 0xffff },
    );
    put_u16(&mut packet, 0x94, 0x7fff);
    put_u16(&mut packet, 0x96, 0xffff);
    put_u32(&mut packet, 0x98, if playing { NORMAL_PITCH } else { 0 });
    packet[0x9d] = if playing { 9 } else { u8::from(loaded) };
    packet[0x9e] = if master {
        if grid {
            1
        } else {
            2
        }
    } else {
        0
    };
    packet[0x9f] = handoff.filter(|n| *n > 0 && *n != 0xff).unwrap_or(0xff);
    put_u32(&mut packet, 0xa0, if grid { clock.beat } else { u32::MAX });
    put_u16(&mut packet, 0xa4, 0x01ff); // No following memory cue countdown.
    packet[0xa6] = if grid { beat_in_bar(clock.beat) } else { 0 };
    packet[0xb6] = 1;
    put_u32(&mut packet, 0xc0, NORMAL_PITCH);
    put_u32(&mut packet, 0xc4, if playing { NORMAL_PITCH } else { 0 });
    put_u32(&mut packet, 0xc8, packet_counter);
    packet[0xcc] = 0x0f;
    packet
}

pub(crate) fn encode_beat(identity: &WireIdentity, clock: &LocalClock) -> Vec<u8> {
    let mut packet = status_header(identity, 0x28, 0, 0x60);
    let bpm = bpm_word(clock.bpm);
    let beat = beat_in_bar(clock.beat);
    let phase = if clock.beat_phase.is_finite() {
        clock.beat_phase.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let bar_beats = if beat == 0 { 4.0 } else { f64::from(5 - beat) };
    let upcoming = [1.0, 2.0, bar_beats, 4.0, bar_beats + 4.0, 8.0];
    for (index, beats) in upcoming.into_iter().enumerate() {
        let millis = if bpm != 0xffff && clock.grid_available {
            ((beats - phase) * 60_000.0 / clock.bpm)
                .round()
                .clamp(0.0, f64::from(u32::MAX - 1)) as u32
        } else {
            u32::MAX
        };
        put_u32(&mut packet, 0x24 + index * 4, millis);
    }
    packet[0x3c..0x54].fill(0xff);
    put_u32(&mut packet, 0x54, NORMAL_PITCH);
    put_u16(&mut packet, 0x5a, bpm);
    packet[0x5c] = if clock.grid_available { beat } else { 0 };
    packet[0x5f] = identity.number;
    packet
}

pub(crate) fn encode_master_request(identity: &WireIdentity) -> Vec<u8> {
    let mut packet = status_header(identity, 0x26, 0, 0x28);
    packet[0x27] = identity.number;
    packet
}

pub(crate) fn encode_master_response(identity: &WireIdentity) -> Vec<u8> {
    let mut packet = status_header(identity, 0x27, 0, 0x2c);
    packet[0x27] = identity.number;
    put_u32(&mut packet, 0x28, 1);
    packet
}

/// Announce the source on UDP 50002, before other library traffic and in reply
/// to a player's 0x10 handshake. The type is 0x11, not 0x17: issue #5's old
/// hexadecimal dump has a missing nibble before the ASCII device name. The
/// independent OPUS-QUAD packet analysis confirms 0x11 and the same layout.
pub(crate) fn encode_library_hello(identity: &WireIdentity) -> Vec<u8> {
    let mut packet = status_header(identity, 0x11, 1, 0x128);
    packet[0x24] = identity.number;
    packet[0x25] = 1;
    put_utf16(&mut packet[0x28..], "Conduction");
    packet
}

/// Announce library availability once to a peer, following the type-0x11 hello.
/// The six settings bytes describe the source's waveform/key presentation.
/// They are explicit because there is no documented preserve-settings value.
pub(crate) fn encode_library_activation(identity: &WireIdentity, dev_settings: [u8; 6]) -> Vec<u8> {
    let mut packet = status_header(identity, 0x47, 1, 0x48);
    packet[0x24] = identity.number;
    packet[0x25] = 4; // Collection slot.
    put_u32(&mut packet, 0x28, 0x1234_5678);
    packet[0x2f] = 1;
    packet[0x30..0x36].copy_from_slice(&dev_settings);
    packet
}

/// Compact export-source status. Unlike the other status packet formats,
/// the published type-0x16 wire format leaves the nominal length word zero.
pub(crate) fn encode_library_status(identity: &WireIdentity) -> Vec<u8> {
    let mut packet = status_header(identity, 0x16, 1, 0x30);
    put_u16(&mut packet, 0x22, 0);
    packet
}

/// rekordbox collection media advertisement (slot 4), also used for queries.
pub(crate) fn encode_library_media(
    identity: &WireIdentity,
    track_count: usize,
    playlist_count: usize,
) -> Vec<u8> {
    let mut packet = status_header(identity, 0x06, 1, 0xc0);
    put_u32(&mut packet, 0x24, u32::from(identity.number));
    put_u32(&mut packet, 0x28, 4);
    put_utf16(&mut packet[0x2c..0x6c], "Conduction");
    put_u16(
        &mut packet,
        0xa6,
        track_count.min(usize::from(u16::MAX)) as u16,
    );
    packet[0xa8] = 6; // Aqua media color.
    packet[0xaa] = 1; // rekordbox-analyzed database.
    put_u16(
        &mut packet,
        0xae,
        playlist_count.min(usize::from(u16::MAX)) as u16,
    );
    packet
}

fn valid_magic(packet: &[u8]) -> bool {
    packet.get(..10) == Some(MAGIC.as_slice())
}

/// Every parser verifies both the actual and declared length before indexing.
fn valid_discovery(packet: &[u8], minimum: usize) -> bool {
    valid_magic(packet)
        && packet.len() >= minimum.max(HEADER_LEN)
        && packet[0x20] == 1
        && matches!(packet[0x21], 2..=4)
        && get_u16(packet, 0x22).map(usize::from) == Some(packet.len())
}

fn valid_status(packet: &[u8], minimum: usize, total_length: bool) -> bool {
    valid_magic(packet)
        && packet.len() >= minimum.max(HEADER_LEN)
        && packet[0x1f] == 1
        && packet[0x21] != 0
        && get_u16(packet, 0x22).map(usize::from)
            == Some(if total_length {
                packet.len()
            } else {
                packet.len() - HEADER_LEN
            })
}

pub(crate) fn parse_discovery(packet: &[u8]) -> Option<DiscoveryPacket> {
    if !valid_discovery(packet, HEADER_LEN) {
        return None;
    }
    match packet[0x0a] {
        0x06 if valid_discovery(packet, 0x36) && packet[0x24] != 0 => {
            Some(DiscoveryPacket::Keepalive {
                number: packet[0x24],
                name: read_name(&packet[0x0c..0x20]),
                mac: packet[0x26..0x2c].try_into().ok()?,
                ip: Ipv4Addr::new(packet[0x2c], packet[0x2d], packet[0x2e], packet[0x2f]),
                kind: packet[0x34],
            })
        }
        0x02 if valid_discovery(packet, 0x32) && packet[0x2e] != 0 && packet[0x30] == 4 => {
            Some(DiscoveryPacket::Keepalive {
                number: packet[0x2e],
                name: read_name(&packet[0x0c..0x20]),
                mac: packet[0x28..0x2e].try_into().ok()?,
                ip: Ipv4Addr::new(packet[0x24], packet[0x25], packet[0x26], packet[0x27]),
                kind: 4,
            })
        }
        0x02 if valid_discovery(packet, 0x32) && packet[0x2e] != 0 => {
            Some(DiscoveryPacket::Claim {
                number: packet[0x2e],
            })
        }
        0x04 if valid_discovery(packet, 0x26) && packet[0x24] != 0 => {
            Some(DiscoveryPacket::Claim {
                number: packet[0x24],
            })
        }
        0x08 if valid_discovery(packet, 0x29) && packet[0x24] != 0 => {
            Some(DiscoveryPacket::Conflict {
                number: packet[0x24],
            })
        }
        0x03 if valid_discovery(packet, 0x27) && packet[0x24] != 0 => match packet[0x26] {
            // An IdUseReply refuses an occupied number. It is distinct from
            // the mixer's channel assignment, despite sharing packet type 3.
            1 => Some(DiscoveryPacket::Conflict {
                number: packet[0x24],
            }),
            0 => Some(DiscoveryPacket::Assignment {
                number: packet[0x24],
            }),
            _ => None,
        },
        0x05 if valid_discovery(packet, 0x26) => Some(DiscoveryPacket::AssignmentFinished),
        0x01 if valid_discovery(packet, 0x2f) => Some(DiscoveryPacket::AssignIntent),
        _ => None,
    }
}

fn effective_bpm(packet: &[u8], bpm_offset: usize, pitch_offset: usize) -> Option<f64> {
    let bpm = get_u16(packet, bpm_offset)?;
    let pitch = get_u32(packet, pitch_offset)?;
    if bpm == 0 || bpm == 0xffff || pitch == 0 {
        return None;
    }
    let result = f64::from(bpm) / 100.0 * f64::from(pitch) / f64::from(NORMAL_PITCH);
    // The pitch format permits up to +100%; reject corrupt high bits instead
    // of allowing a malformed UDP packet to request an extreme local tempo.
    (pitch <= 2 * NORMAL_PITCH).then_some(result)
}

pub(crate) fn parse_beat(packet: &[u8]) -> Option<BeatPacket> {
    if !valid_status(packet, 0x60, false)
        || packet[0x0a] != 0x28
        || packet[0x20] != 0
        || packet[0x21] != packet[0x5f]
        || !(1..=4).contains(&packet[0x5c])
    {
        return None;
    }
    Some(BeatPacket {
        number: packet[0x21],
        bpm: effective_bpm(packet, 0x5a, 0x54)?,
        beat: packet[0x5c],
    })
}

fn handoff_number(value: u8) -> Option<u8> {
    (value > 0 && value != 0xff).then_some(value)
}

pub(crate) fn parse_status(packet: &[u8]) -> Option<StatusPacket> {
    if packet.len() < HEADER_LEN || !valid_magic(packet) {
        return None;
    }
    match packet[0x0a] {
        0x0a if matches!(packet[0x20], 3..=6)
            && valid_status(packet, 0xd0, false)
            && packet[0x24] == packet[0x21] =>
        {
            let flags = packet[0x89];
            let beat = get_u32(packet, 0xa0)?;
            let track = get_u32(packet, 0x2c)?;
            Some(StatusPacket {
                number: packet[0x21],
                name: read_name(&packet[0x0b..0x1f]),
                bpm: effective_bpm(packet, 0x92, 0x8c),
                playing: if flags == 0 {
                    matches!(packet[0x7b], 3 | 4 | 7 | 9 | 0x12)
                } else {
                    flags & 0x40 != 0
                },
                synced: flags & 0x10 != 0,
                master: flags & 0x20 != 0,
                handoff: handoff_number(packet[0x9f]),
                sync_counter: get_u32(packet, 0x84)?,
                beat: if beat == u32::MAX { 0 } else { beat },
                track_id: (packet[0x2a] != 0 && track != 0).then_some(track),
            })
        }
        0x29 if matches!(packet[0x20], 0 | 1)
            && valid_status(packet, 0x38, packet[0x20] == 1)
            && packet[0x24] == packet[0x21] =>
        {
            let flags = packet[0x27];
            Some(StatusPacket {
                number: packet[0x21],
                name: read_name(&packet[0x0b..0x1f]),
                bpm: effective_bpm(packet, 0x2e, 0x28),
                playing: flags & 0x40 != 0,
                synced: flags & 0x10 != 0,
                master: flags & 0x20 != 0,
                handoff: handoff_number(packet[0x36]),
                sync_counter: 0,
                beat: 0, // Mixers do not report an absolute beat counter.
                track_id: None,
            })
        }
        _ => None,
    }
}

pub(crate) fn parse_control(packet: &[u8]) -> Option<ControlPacket> {
    if !valid_status(packet, 0x28, false) || packet[0x20] != 0 || packet[0x27] != packet[0x21] {
        return None;
    }
    match packet[0x0a] {
        0x26 => Some(ControlPacket::MasterRequest {
            number: packet[0x21],
        }),
        0x27 if packet.len() >= 0x2c => Some(ControlPacket::MasterResponse {
            number: packet[0x21],
            accepted: get_u32(packet, 0x28)? == 1,
        }),
        0x2a if packet.len() >= 0x2c => match get_u32(packet, 0x28)? {
            0x10 => Some(ControlPacket::Sync(true)),
            0x20 => Some(ControlPacket::Sync(false)),
            1 => Some(ControlPacket::BecomeMaster),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> WireIdentity {
        WireIdentity {
            number: 2,
            name: "Conduction".into(),
            ip: Ipv4Addr::new(192, 168, 1, 9),
            mac: [2, 3, 4, 5, 6, 7],
        }
    }

    fn clock() -> LocalClock {
        LocalClock {
            deck: "A".into(),
            playing: true,
            bpm: 128.5,
            beat: 7,
            beat_phase: 0.0,
            track_id: Some(47),
            synced: true,
            grid_available: true,
        }
    }

    fn fixture(hex: &str) -> Vec<u8> {
        hex.split_whitespace()
            .map(|word| u8::from_str_radix(word, 16).expect("valid external fixture byte"))
            .collect()
    }

    #[test]
    fn parses_original_hardware_payloads_and_distinguishes_id_refusal() {
        let final_claim = fixture(include_str!("../tests/fixtures/wire-cdj-final-claim.hex"));
        assert_eq!(final_claim.len(), 38);
        assert_eq!(
            parse_discovery(&final_claim),
            Some(DiscoveryPacket::Claim { number: 3 })
        );
        let id = WireIdentity {
            number: 3,
            name: "CDJ-2000nexus".into(),
            ..identity()
        };
        let mut encoded = encode_claim_final(&id, 1);
        // Only the documented CDJ-3000 structure/version byte differs.
        encoded[0x21] = 2;
        assert_eq!(encoded, final_claim);

        let assignment = fixture(include_str!("../tests/fixtures/wire-mixer-assignment.hex"));
        let refusal = fixture(include_str!("../tests/fixtures/wire-xdj-id-refusal.hex"));
        assert_eq!(
            parse_discovery(&assignment),
            Some(DiscoveryPacket::Assignment { number: 3 })
        );
        assert_eq!(
            parse_discovery(&refusal),
            Some(DiscoveryPacket::Conflict { number: 4 })
        );

        let keepalive = fixture(include_str!("../tests/fixtures/wire-cdj-keepalive.hex"));
        assert_eq!(
            parse_discovery(&keepalive),
            Some(DiscoveryPacket::Keepalive {
                number: 2,
                name: "CDJ-2000nexus".into(),
                ip: Ipv4Addr::new(169, 254, 244, 181),
                mac: [0x74, 0x5e, 0x1c, 0x56, 0xf4, 0xb5],
                kind: 1,
            })
        );
        let beat = fixture(include_str!("../tests/fixtures/wire-mixer-beat.hex"));
        assert_eq!(
            parse_beat(&beat),
            Some(BeatPacket {
                number: 33,
                bpm: 120.0,
                beat: 4
            })
        );
        let status = fixture(include_str!("../tests/fixtures/wire-cdj-status.hex"));
        let parsed = parse_status(&status).expect("real nexus status");
        assert_eq!(parsed.number, 2);
        assert_eq!(parsed.track_id, Some(50));
        assert_eq!(parsed.bpm, Some(128.0));
        assert!(parsed.synced);
        assert!(!parsed.playing && !parsed.master);
        for packet in [&final_claim, &assignment, &refusal, &keepalive] {
            for end in 0..packet.len() {
                assert!(parse_discovery(&packet[..end]).is_none());
            }
        }
        for end in 0..beat.len() {
            assert!(parse_beat(&beat[..end]).is_none());
        }
        for end in 0..status.len() {
            assert!(parse_status(&status[..end]).is_none());
        }
    }

    #[test]
    fn export_source_matches_independently_transcribed_interface_vectors() {
        // These are external interface observations, not captured export
        // hardware. See wire-provenance.md for the important distinction.
        let id = WireIdentity {
            number: 17,
            name: "rekordbox".into(),
            ..identity()
        };
        let keepalive = fixture(include_str!(
            "../tests/fixtures/wire-source-keepalive-reference.hex"
        ));
        assert_eq!(encode_keepalive(&id, 1, true, true), keepalive);
        assert_eq!(encode_keepalive(&id, 9, false, true), keepalive);
        assert_eq!(
            parse_discovery(&keepalive),
            Some(DiscoveryPacket::Keepalive {
                number: 17,
                name: "rekordbox".into(),
                ip: id.ip,
                mac: id.mac,
                kind: 4,
            })
        );
        let hello_prefix = fixture(include_str!(
            "../tests/fixtures/wire-source-hello-prefix-reference.hex"
        ));
        assert_eq!(&encode_library_hello(&id)[..40], hello_prefix);
        let activation = fixture(include_str!(
            "../tests/fixtures/wire-source-activation-reference.hex"
        ));
        assert_eq!(
            encode_library_activation(&id, [1, 2, 3, 1, 2, 1]),
            activation
        );
        let status = fixture(include_str!(
            "../tests/fixtures/wire-source-status-reference.hex"
        ));
        assert_eq!(encode_library_status(&id), status);
        for end in 0..keepalive.len() {
            assert!(parse_discovery(&keepalive[..end]).is_none());
        }
    }

    #[test]
    fn source_media_fields_match_original_export_capture() {
        let recorded = fixture(include_str!(
            "../tests/fixtures/wire-source-media-capture.hex"
        ));
        let id = WireIdentity {
            number: 17,
            name: "rekordbox".into(),
            ..identity()
        };
        let encoded = encode_library_media(&id, 6, 0);
        // Labels, tint and My Settings availability are source-specific. The
        // source/slot routing, type and count fields must match the capture.
        assert_eq!(recorded.len(), 192);
        assert_eq!(&encoded[..0x2c], &recorded[..0x2c]);
        assert_eq!(&encoded[0xa6..0xa8], &recorded[0xa6..0xa8]);
        assert_eq!(encoded[0xaa], recorded[0xaa]);
        assert_eq!(&encoded[0xae..0xb0], &recorded[0xae..0xb0]);
    }

    #[test]
    fn cdj3000_discovery_uses_documented_subtypes_and_trailers() {
        let id = identity();
        let initial = encode_initial(&id);
        assert_eq!(initial.len(), 0x26);
        assert_eq!(&initial[0x20..], &[1, 4, 0, 0x26, 1, 0x40]);
        let first = encode_claim1(&id, 2);
        assert_eq!(&first[0x20..], &[1, 3, 0, 0x2c, 2, 1, 2, 3, 4, 5, 6, 7]);
        for packet in [encode_claim2(&id, 1), encode_claim_final(&id, 2)] {
            assert_eq!(packet[0x21], 3);
            assert_eq!(
                parse_discovery(&packet),
                Some(DiscoveryPacket::Claim { number: 2 })
            );
        }
        let keepalive = encode_keepalive(&id, 3, true, false);
        assert_eq!(keepalive[0x25], 2);
        assert_eq!(keepalive[0x30], 3);
        assert_eq!(&keepalive[0x34..], &[1, 0x64]);
        assert_eq!(
            parse_discovery(&keepalive),
            Some(DiscoveryPacket::Keepalive {
                number: 2,
                name: id.name.clone(),
                ip: id.ip,
                mac: id.mac,
                kind: 1
            })
        );
        let library = WireIdentity {
            number: 17,
            name: "rekordbox".into(),
            ..id
        };
        assert!(matches!(
            parse_discovery(&encode_keepalive(&library, 2, false, true)),
            Some(DiscoveryPacket::Keepalive {
                number: 17,
                kind: 4,
                ..
            })
        ));
    }

    #[test]
    fn status_carries_audio_clock_and_master_handoff() {
        let packet = encode_status(&identity(), &clock(), true, Some(4), 99, 100);
        let status = parse_status(&packet).unwrap();
        assert_eq!(packet.len(), 0xd4);
        assert_eq!(get_u16(&packet, 0x22), Some(0xb0));
        assert_eq!(packet[0x28], 17);
        assert_eq!(packet[0x29], 4);
        assert_eq!(packet[0xa6], 3);
        assert_eq!(get_u32(&packet, 0xc8), Some(100));
        assert_eq!(
            status,
            StatusPacket {
                number: 2,
                name: "Conduction".into(),
                bpm: Some(128.5),
                playing: true,
                synced: true,
                master: true,
                handoff: Some(4),
                sync_counter: 99,
                beat: 7,
                track_id: Some(47)
            }
        );
        let mut paused = clock();
        paused.playing = false;
        assert!(
            !parse_status(&encode_status(&identity(), &paused, false, None, 0, 0))
                .unwrap()
                .playing
        );
        let empty = parse_status(&encode_status(
            &identity(),
            &LocalClock::default(),
            false,
            None,
            0,
            0,
        ))
        .unwrap();
        assert_eq!(empty.bpm, None);
        assert_eq!(empty.track_id, None);
        assert_eq!(empty.beat, 0);
        assert!(!empty.playing && !empty.synced);
    }

    #[test]
    fn beat_timing_bar_position_and_effective_pitch() {
        let mut clock = clock();
        clock.bpm = 120.0;
        clock.beat = 3;
        clock.beat_phase = 0.25;
        let mut packet = encode_beat(&identity(), &clock);
        for (index, expected) in [375, 875, 875, 1875, 2875, 3875].into_iter().enumerate() {
            assert_eq!(get_u32(&packet, 0x24 + index * 4), Some(expected));
        }
        assert_eq!(
            parse_beat(&packet),
            Some(BeatPacket {
                number: 2,
                bpm: 120.0,
                beat: 3
            })
        );
        put_u32(&mut packet, 0x54, NORMAL_PITCH * 3 / 2);
        assert_eq!(parse_beat(&packet).unwrap().bpm, 180.0);
        put_u16(&mut packet, 0x5a, 0xffff);
        assert_eq!(parse_beat(&packet), None);
    }

    #[test]
    fn control_handoff_and_sync_commands() {
        let id = identity();
        assert_eq!(
            parse_control(&encode_master_request(&id)),
            Some(ControlPacket::MasterRequest { number: 2 })
        );
        assert_eq!(
            parse_control(&encode_master_response(&id)),
            Some(ControlPacket::MasterResponse {
                number: 2,
                accepted: true
            })
        );
        for (value, expected) in [
            (0x10, ControlPacket::Sync(true)),
            (0x20, ControlPacket::Sync(false)),
            (1, ControlPacket::BecomeMaster),
        ] {
            let mut packet = status_header(&id, 0x2a, 0, 0x2c);
            packet[0x27] = 2;
            put_u32(&mut packet, 0x28, value);
            assert_eq!(parse_control(&packet), Some(expected));
        }
    }

    #[test]
    fn accepts_mixer_handoff_and_rekordbox_length_variant() {
        let id = WireIdentity {
            number: 33,
            ..identity()
        };
        let mut packet = status_header(&id, 0x29, 0, 0x38);
        packet[0x24] = 33;
        packet[0x27] = 0xf0;
        put_u32(&mut packet, 0x28, NORMAL_PITCH);
        put_u16(&mut packet, 0x2e, 12000);
        packet[0x36] = 2;
        let status = parse_status(&packet).unwrap();
        assert_eq!(status.bpm, Some(120.0));
        assert!(status.master);
        assert_eq!(status.handoff, Some(2));
        packet[0x20] = 1;
        put_u16(&mut packet, 0x22, 0x38);
        assert_eq!(parse_status(&packet), Some(status));
    }

    #[test]
    fn library_handshake_matches_capture_lengths_and_media_offsets() {
        let id = WireIdentity {
            number: 17,
            name: "rekordbox".into(),
            ..identity()
        };
        let hello = encode_library_hello(&id);
        assert_eq!(hello.len(), 296);
        assert_eq!(&hello[0x20..0x28], &[1, 17, 1, 4, 17, 1, 0, 0]);
        assert_eq!(&hello[0x28..0x2c], &[0, b'C', 0, b'o']);
        let media = encode_library_media(&id, 60_000, 70_000);
        assert_eq!(media.len(), 192);
        assert_eq!(
            &media[0x20..0x2c],
            &[1, 17, 0, 0x9c, 0, 0, 0, 17, 0, 0, 0, 4]
        );
        assert_eq!(get_u16(&media, 0xa6), Some(60_000));
        assert_eq!(get_u16(&media, 0xae), Some(u16::MAX));
        assert_eq!(media[0xaa], 1);
        assert_eq!(media[0xab], 0); // No phantom My Settings file.
    }

    #[test]
    fn utf16_and_device_names_do_not_split_codepoints() {
        let mut field = [0; 8];
        put_utf16(&mut field, "日🎵a");
        assert_eq!(&field, &[0x65, 0xe5, 0xd8, 0x3c, 0xdf, 0xb5, 0, 0]);
        let id = WireIdentity {
            name: "Conduction 日本語".into(),
            ..identity()
        };
        assert!(
            matches!(parse_discovery(&encode_keepalive(&id, 1, true, false)), Some(DiscoveryPacket::Keepalive { name, .. }) if name == "Conduction ???")
        );
    }

    #[test]
    fn rejects_all_packet_truncations_corrupt_headers_and_lengths() {
        let id = identity();
        let mut discovery = vec![
            encode_claim2(&id, 1),
            encode_claim_final(&id, 1),
            encode_keepalive(&id, 1, false, false),
            encode_assignment_finished(&id),
        ];
        for (kind, size) in [(8, 0x29), (3, 0x27), (1, 0x2f)] {
            let mut packet = discovery_header(&id, kind, 2, size);
            packet[0x24] = 2;
            discovery.push(packet);
        }
        for packet in discovery {
            assert!(parse_discovery(&packet).is_some());
            for end in 0..packet.len() {
                assert!(
                    parse_discovery(&packet[..end]).is_none(),
                    "discovery truncated to {end}"
                );
            }
            let mut bad = packet;
            bad[0] ^= 1;
            assert!(parse_discovery(&bad).is_none());
            bad[0] ^= 1;
            bad[0x23] ^= 1;
            assert!(parse_discovery(&bad).is_none());
        }
        let beat = encode_beat(&id, &clock());
        let status = encode_status(&id, &clock(), true, None, 0, 0);
        for end in 0..beat.len() {
            assert!(parse_beat(&beat[..end]).is_none());
        }
        for end in 0..status.len() {
            assert!(parse_status(&status[..end]).is_none());
        }
        for packet in [encode_master_request(&id), encode_master_response(&id)] {
            for end in 0..packet.len() {
                assert!(parse_control(&packet[..end]).is_none());
            }
        }
        // Crafted packets cannot evade minimum lengths by updating len_r.
        for len in HEADER_LEN..0xd0 {
            let mut shortened = status[..len].to_vec();
            put_u16(&mut shortened, 0x22, (len - HEADER_LEN) as u16);
            assert!(parse_status(&shortened).is_none());
        }
    }
}
