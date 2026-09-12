//! Read-only, bounded Pro DJ Link remote database server.
//!
//! Wire descriptions: https://djl-analysis.deepsymmetry.org/djl-analysis/track_metadata.html
//! and https://djl-analysis.deepsymmetry.org/djl-analysis/menus.html.
//! Path retrieval: https://github.com/Deep-Symmetry/dysentery/issues/5.
//! Numeric item types, cue frame units and UTF-16LE cue comments were cross-checked
//! against Evan Purkhiser's MIT-licensed prolink-connect (2013). This is an
//! independent implementation of the published protocol, not copied source.
//!
//! Only legacy blue waveforms are offered. Unknown analysis tags return an empty
//! result rather than fabricated color/frequency data. Hardware interoperability
//! still requires a CDJ acceptance run, including concurrent NFS loading.

use crate::{LibrarySnapshot, LinkTrack};
use std::collections::{BTreeMap, HashMap};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAGIC: u32 = 0x8723_49ae;
const SOURCE_NUMBER: u32 = 0x11;
const QUERY: &[u8] = b"\0\0\0\x0fRemoteDBServer\0";
const MAX_CLIENTS: usize = 16;
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_BLOB_BYTES: usize = 8 * 1024 * 1024;
const MAX_MENU_ITEMS: usize = 50_000;
const MAX_MENU_CONTEXTS: usize = 8;
const MAX_PAGE: usize = 256;

#[derive(Clone, Debug, PartialEq)]
enum Field {
    Number(u32),
    Text(String),
    Blob(Vec<u8>),
}

impl Field {
    fn number(&self) -> io::Result<u32> {
        match self {
            Self::Number(value) => Ok(*value),
            _ => Err(invalid("expected a numeric argument")),
        }
    }

    fn tag(&self) -> u8 {
        match self {
            Self::Number(_) => 6,
            Self::Text(_) => 2,
            Self::Blob(_) => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Message {
    tx: u32,
    kind: u16,
    args: Vec<Field>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn read_u32(input: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    input.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

fn read_field(input: &mut impl Read, budget: &mut usize) -> io::Result<Field> {
    let mut tag = [0];
    input.read_exact(&mut tag)?;
    let length = match tag[0] {
        0x0f => 1,
        0x10 => 2,
        0x11 => 4,
        0x14 => read_u32(input)? as usize,
        0x26 => (read_u32(input)? as usize)
            .checked_mul(2)
            .ok_or_else(|| invalid("string length overflow"))?,
        _ => return Err(invalid("unknown field type")),
    };
    if length > *budget {
        return Err(invalid("message exceeds size limit"));
    }
    *budget -= length;
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    match tag[0] {
        0x0f..=0x11 => Ok(Field::Number(
            bytes
                .into_iter()
                .fold(0, |value, byte| (value << 8) | u32::from(byte)),
        )),
        0x14 => Ok(Field::Blob(bytes)),
        0x26 => {
            if length < 2 || bytes[length - 2..] != [0, 0] {
                return Err(invalid("unterminated UTF-16 string"));
            }
            let units: Vec<_> = bytes[..length - 2]
                .chunks_exact(2)
                .map(|v| u16::from_be_bytes([v[0], v[1]]))
                .collect();
            let text = String::from_utf16(&units).map_err(|_| invalid("invalid UTF-16"))?;
            if text.contains('\0') {
                return Err(invalid("embedded NUL in string"));
            }
            Ok(Field::Text(text))
        }
        _ => unreachable!(),
    }
}

fn read_message(input: &mut impl Read) -> io::Result<Message> {
    let mut budget = MAX_REQUEST_BYTES;
    if read_field(input, &mut budget)? != Field::Number(MAGIC) {
        return Err(invalid("invalid message magic"));
    }
    let tx = read_field(input, &mut budget)?.number()?;
    let kind = u16::try_from(read_field(input, &mut budget)?.number()?)
        .map_err(|_| invalid("invalid message type"))?;
    let count = read_field(input, &mut budget)?.number()? as usize;
    if count > 12 {
        return Err(invalid("too many arguments"));
    }
    let Field::Blob(tags) = read_field(input, &mut budget)? else {
        return Err(invalid("missing argument tags"));
    };
    if tags.len() != 12 || tags[count..].iter().any(|&tag| tag != 0) {
        return Err(invalid("invalid argument tag vector"));
    }
    let mut args = Vec::with_capacity(count);
    for &tag in &tags[..count] {
        // Empty blobs are listed in the argument tags but omitted on the wire.
        let field = if tag == 3 && args.last() == Some(&Field::Number(0)) {
            Field::Blob(Vec::new())
        } else {
            read_field(input, &mut budget)?
        };
        if field.tag() != tag {
            return Err(invalid("field does not match its argument tag"));
        }
        if let Field::Blob(bytes) = &field {
            let Some(Field::Number(length)) = args.last() else {
                return Err(invalid("binary argument lacks length"));
            };
            if bytes.len() != *length as usize {
                return Err(invalid("binary argument length mismatch"));
            }
        }
        args.push(field);
    }
    Ok(Message { tx, kind, args })
}

fn put_number(bytes: &mut Vec<u8>, value: u32) {
    bytes.push(0x11);
    bytes.extend(value.to_be_bytes());
}

fn encode_message(message: &Message) -> Vec<u8> {
    let mut bytes = Vec::new();
    put_number(&mut bytes, MAGIC);
    put_number(&mut bytes, message.tx);
    bytes.push(0x10);
    bytes.extend(message.kind.to_be_bytes());
    bytes.extend([0x0f, message.args.len() as u8, 0x14, 0, 0, 0, 12]);
    let mut tags = [0; 12];
    for (tag, field) in tags.iter_mut().zip(&message.args) {
        *tag = field.tag();
    }
    bytes.extend(tags);
    for field in &message.args {
        match field {
            Field::Number(value) => put_number(&mut bytes, *value),
            Field::Text(value) => {
                bytes.push(0x26);
                bytes.extend(((value.encode_utf16().count() + 1) as u32).to_be_bytes());
                for unit in value.encode_utf16().chain(std::iter::once(0)) {
                    bytes.extend(unit.to_be_bytes());
                }
            }
            Field::Blob(value) if !value.is_empty() => {
                bytes.push(0x14);
                bytes.extend((value.len() as u32).to_be_bytes());
                bytes.extend(value);
            }
            Field::Blob(_) => {}
        }
    }
    bytes
}

fn response(request: &Message, kind: u16, args: Vec<Field>) -> Message {
    Message {
        tx: request.tx,
        kind,
        args,
    }
}

fn ack(request: &Message, count: u32) -> Message {
    response(
        request,
        0x4000,
        vec![Field::Number(u32::from(request.kind)), Field::Number(count)],
    )
}

fn error(request: &Message) -> Message {
    response(
        request,
        0x4003,
        vec![Field::Number(u32::from(request.kind)), Field::Number(0)],
    )
}

fn argument(request: &Message, index: usize) -> io::Result<u32> {
    request
        .args
        .get(index)
        .ok_or_else(|| invalid("missing argument"))?
        .number()
}

fn label(value: &str) -> String {
    value.chars().filter(|&c| c != '\0').take(1024).collect()
}

#[derive(Clone, Debug)]
struct MenuItem {
    parent: u32,
    id: u32,
    first: String,
    second: String,
    kind: u32,
    position: u32,
    flags: u32,
    track_type: u32,
}

impl MenuItem {
    fn new(kind: u32, id: u32, first: &str) -> Self {
        Self {
            parent: 0,
            id,
            first: label(first),
            second: String::new(),
            kind,
            position: 0,
            flags: 0,
            track_type: 0,
        }
    }

    fn message(&self, request: &Message) -> Message {
        response(
            request,
            0x4101,
            vec![
                Field::Number(self.parent),
                Field::Number(self.id),
                Field::Number(((self.first.encode_utf16().count() + 1) * 2) as u32),
                Field::Text(self.first.clone()),
                Field::Number(((self.second.encode_utf16().count() + 1) * 2) as u32),
                Field::Text(self.second.clone()),
                Field::Number(self.kind),
                Field::Number(self.flags),
                Field::Number(0),
                Field::Number(self.position),
                Field::Number(self.track_type),
                Field::Number(0),
            ],
        )
    }
}

// Category identifiers only need to remain stable for a connected browsing
// session. They are derived from the complete sorted category names, not hashes
// (which would silently merge colliding artists).
fn categories(library: &LibrarySnapshot, column: fn(&LinkTrack) -> &str) -> BTreeMap<String, u32> {
    let mut result = BTreeMap::new();
    for track in &library.tracks {
        result.entry(column(track).to_owned()).or_insert(0);
    }
    for (index, id) in result.values_mut().enumerate() {
        *id = index as u32 + 1;
    }
    result
}

fn artist(track: &LinkTrack) -> &str {
    &track.artist
}
fn album(track: &LinkTrack) -> &str {
    &track.album
}
fn genre(track: &LinkTrack) -> &str {
    &track.genre
}

fn bpm(value: f64) -> u32 {
    if value.is_finite() && value > 0.0 {
        (value * 100.0).round().min(65534.0) as u32
    } else {
        0
    }
}

fn track_item(track: &LinkTrack, artists: &BTreeMap<String, u32>) -> MenuItem {
    let mut item = MenuItem::new(0x0704, track.id, &track.title);
    item.parent = artists.get(&track.artist).copied().unwrap_or(0);
    item.second = label(&track.artist);
    // Every rekordbox track entry in the independent S05/S20 captures carries
    // these values; their bit-level semantics are not yet documented.
    item.flags = 0x0100_0000;
    item.track_type = 0x100;
    item
}

/// The only path exposed by path metadata; never reveal a host filesystem path.
pub(crate) fn exported_track_path(track: &LinkTrack) -> String {
    crate::nfs::export_path(track)
}

fn metadata(track: &LinkTrack, artists: &BTreeMap<String, u32>, path: bool) -> Vec<MenuItem> {
    let mut title = track_item(track, artists);
    title.kind = 0x0004;
    title.second.clear();
    if path {
        // S13 hardware capture: type4 carries a file format, and the path
        // entry's parent field carries the audio byte length.
        let format = match track
            .path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "mp3" => 1,
            "aac" | "m4a" => 4,
            "flac" => 5,
            "wav" | "wave" => 11,
            "aif" | "aiff" => 12,
            _ => 0,
        };
        let mut path_item = MenuItem::new(0, track.id, &exported_track_path(track));
        path_item.parent = track.byte_size.min(u64::from(u32::MAX)) as u32;
        return vec![
            MenuItem::new(4, format, ""),
            MenuItem::new(0x0b, track.duration_ms / 1000, ""),
            MenuItem::new(0x0d, bpm(track.bpm), ""),
            MenuItem::new(0x23, track.id, ""),
            path_item,
            MenuItem::new(0x2f, 1, ""),
        ];
    }
    vec![
        title,
        MenuItem::new(
            7,
            artists.get(&track.artist).copied().unwrap_or(0),
            &track.artist,
        ),
        MenuItem::new(2, 0, &track.album),
        MenuItem::new(0x0b, track.duration_ms / 1000, ""),
        MenuItem::new(0x0d, bpm(track.bpm), ""),
        MenuItem::new(0x23, 0, ""),
        MenuItem::new(0x0f, 0, ""),
        MenuItem::new(0x0a, 0, ""),
        MenuItem::new(0x13, 0, ""),
        MenuItem::new(6, 0, &track.genre),
        MenuItem::new(0x2e, 0, ""),
    ]
}

fn menu(request: &Message, library: &LibrarySnapshot) -> io::Result<Option<Vec<MenuItem>>> {
    let artists = categories(library, artist);
    let albums = categories(library, album);
    let genres = categories(library, genre);
    let mut tracks: Vec<&LinkTrack> = library.tracks.iter().take(MAX_MENU_ITEMS).collect();
    let sort = argument(request, 1).unwrap_or(0);
    let mut preserve_order = false;
    match request.kind {
        0x1000 => {
            return Ok(Some(vec![
                MenuItem::new(0x83, 4, "\u{fffa}TRACK\u{fffb}"),
                MenuItem::new(0x81, 2, "\u{fffa}ARTIST\u{fffb}"),
                MenuItem::new(0x82, 3, "\u{fffa}ALBUM\u{fffb}"),
                MenuItem::new(0x84, 5, "\u{fffa}PLAYLIST\u{fffb}"),
                MenuItem::new(0x91, 18, "\u{fffa}SEARCH\u{fffb}"),
            ]))
        }
        0x1400 => {
            return Ok(Some(vec![
                MenuItem::new(0xa1, 0, "\u{fffa}DEFAULT\u{fffb}"),
                MenuItem::new(0xa2, 1, "\u{fffa}ALPHABET\u{fffb}"),
                MenuItem::new(0x81, 2, "\u{fffa}ARTIST\u{fffb}"),
                MenuItem::new(0x82, 3, "\u{fffa}ALBUM\u{fffb}"),
                MenuItem::new(0x85, 4, "\u{fffa}BPM\u{fffb}"),
                MenuItem::new(0x80, 6, "\u{fffa}GENRE\u{fffb}"),
            ]))
        }
        0x1001..=0x1003 => {
            let (items, kind) = match request.kind {
                0x1001 => (&genres, 6),
                0x1002 => (&artists, 7),
                _ => (&albums, 2),
            };
            return Ok(Some(
                items
                    .iter()
                    .take(MAX_MENU_ITEMS)
                    .map(|(name, id)| MenuItem::new(kind, *id, name))
                    .collect(),
            ));
        }
        0x1102 => {
            let id = argument(request, 2)?;
            let mut names = BTreeMap::new();
            for track in &tracks {
                if id == u32::MAX || artists.get(&track.artist) == Some(&id) {
                    names.insert(
                        track.album.as_str(),
                        albums.get(&track.album).copied().unwrap_or(0),
                    );
                }
            }
            let mut items = vec![MenuItem::new(0xa0, u32::MAX, "ALL")];
            items.extend(
                names
                    .into_iter()
                    .map(|(name, id)| MenuItem::new(2, id, name)),
            );
            return Ok(Some(items));
        }
        0x1103 => {
            let id = argument(request, 2)?;
            tracks.retain(|track| id == u32::MAX || albums.get(&track.album) == Some(&id));
        }
        0x1202 => {
            let artist_id = argument(request, 2)?;
            let album_id = argument(request, 3)?;
            tracks.retain(|track| {
                (artist_id == u32::MAX || artists.get(&track.artist) == Some(&artist_id))
                    && (album_id == u32::MAX || albums.get(&track.album) == Some(&album_id))
            });
        }
        0x1105 => {
            let id = argument(request, 2)?;
            if argument(request, 3)? != 0 {
                return Ok(Some(if id == 0 {
                    library
                        .playlists
                        .iter()
                        .take(MAX_MENU_ITEMS)
                        .map(|playlist| MenuItem::new(8, playlist.id, &playlist.name))
                        .collect()
                } else {
                    Vec::new()
                }));
            }
            let Some(playlist) = library.playlists.iter().find(|p| p.id == id) else {
                return Ok(None);
            };
            let by_id: HashMap<_, _> = tracks.iter().map(|track| (track.id, *track)).collect();
            tracks = playlist
                .track_ids
                .iter()
                .take(MAX_MENU_ITEMS)
                .filter_map(|id| by_id.get(id).copied())
                .collect();
            preserve_order = sort == 0;
        }
        0x1300 => {
            let Some(Field::Text(query)) = request.args.get(3) else {
                return Err(invalid("missing search string"));
            };
            if argument(request, 2)? as usize != (query.encode_utf16().count() + 1) * 2 {
                return Err(invalid("search string length mismatch"));
            }
            let query = query.to_lowercase();
            tracks.retain(|track| {
                [&track.title, &track.artist, &track.album]
                    .iter()
                    .any(|value| value.to_lowercase().contains(&query))
            });
        }
        0x2002 | 0x2202 | 0x2102 => {
            let id = argument(request, 1)?;
            return Ok(library
                .tracks
                .iter()
                .find(|track| track.id == id)
                .map(|track| metadata(track, &artists, request.kind == 0x2102)));
        }
        0x1004 => {}
        _ => return Err(invalid("unsupported menu")),
    }
    if !preserve_order {
        tracks.sort_by(|a, b| {
            match sort {
                2 => a.artist.to_lowercase().cmp(&b.artist.to_lowercase()),
                3 => a.album.to_lowercase().cmp(&b.album.to_lowercase()),
                4 => a.bpm.total_cmp(&b.bpm),
                6 => a.genre.to_lowercase().cmp(&b.genre.to_lowercase()),
                8 => a.duration_ms.cmp(&b.duration_ms),
                _ => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            }
            .then_with(|| a.id.cmp(&b.id))
        });
    }
    Ok(Some(
        tracks
            .into_iter()
            .enumerate()
            .map(|(index, track)| {
                let mut item = track_item(track, &artists);
                item.position = index as u32 + 1;
                match sort {
                    3 => {
                        item.kind = 0x0204;
                        item.parent = albums.get(&track.album).copied().unwrap_or(0);
                        item.second = label(&track.album);
                    }
                    4 => {
                        item.kind = 0x0d04;
                        item.parent = bpm(track.bpm);
                        item.second.clear();
                    }
                    6 => {
                        item.kind = 0x0604;
                        item.parent = genres.get(&track.genre).copied().unwrap_or(0);
                        item.second = label(&track.genre);
                    }
                    8 => {
                        item.kind = 0x0b04;
                        item.parent = track.duration_ms / 1000;
                        item.second.clear();
                    }
                    _ => {}
                }
                item
            })
            .collect(),
    ))
}

fn beat_grid(track: &LinkTrack) -> Vec<u8> {
    if track.beats.is_empty() {
        return Vec::new();
    }
    let count = track.beats.len().min((MAX_BLOB_BYTES - 20) / 16);
    let mut bytes = vec![0; 20 + count * 16];
    // S06/S13 hardware replies expose the count and entry byte length here.
    // The final four header bytes vary; their meaning remains unknown.
    bytes[..4].copy_from_slice(&[0, 0, 8, 0]);
    bytes[4..8].copy_from_slice(&(count as u32).to_le_bytes());
    bytes[8..12].copy_from_slice(&((count * 16) as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
    for (index, (beat, entry)) in track
        .beats
        .iter()
        .zip(bytes[20..].chunks_exact_mut(16))
        .enumerate()
    {
        let within_bar = if (1..=4).contains(&beat.beat) {
            beat.beat
        } else {
            (index % 4 + 1) as u8
        };
        entry[..2].copy_from_slice(&u16::from(within_bar).to_le_bytes());
        entry[2..4].copy_from_slice(&(bpm(beat.bpm) as u16).to_le_bytes());
        entry[4..8].copy_from_slice(&beat.time_ms.to_le_bytes());
        entry[8..16].fill(0xff);
    }
    bytes
}

fn frames(milliseconds: u32) -> u32 {
    ((u64::from(milliseconds) * 150 + 500) / 1000).min(u64::from(u32::MAX)) as u32
}

fn cues(track: &LinkTrack, extended: bool) -> (Vec<u8>, u32, u32) {
    let mut bytes = Vec::new();
    let mut hot = 0;
    let mut memory = 0;
    for cue in track.cues.iter().take(1024) {
        if cue.slot > if extended { 8 } else { 3 } {
            continue;
        }
        if cue.slot == 0 {
            memory += 1;
        } else {
            hot += 1;
        }
        let is_loop = cue.end_ms.is_some_and(|end| end > cue.time_ms);
        let mut entry = vec![0; if extended { 0x4a } else { 0x24 }];
        if extended {
            entry[4] = cue.slot;
            entry[6] = if is_loop { 2 } else { 1 };
            let comment: Vec<u8> = label(&cue.name)
                .encode_utf16()
                .chain(std::iter::once(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            entry[0x48..0x4a].copy_from_slice(&(comment.len() as u16).to_le_bytes());
            entry.extend(comment);
            entry.extend([0; 8]);
            let length = entry.len() as u32;
            entry[..4].copy_from_slice(&length.to_le_bytes());
        } else {
            entry[0] = u8::from(is_loop);
            entry[1] = 1;
            entry[2] = cue.slot;
        }
        entry[0x0c..0x10].copy_from_slice(&frames(cue.time_ms).to_le_bytes());
        entry[0x10..0x14].copy_from_slice(&frames(cue.end_ms.unwrap_or(0)).to_le_bytes());
        bytes.extend(entry);
    }
    (bytes, hot, memory)
}

fn binary(request: &Message, kind: u16, data: Vec<u8>) -> Message {
    response(
        request,
        kind,
        vec![
            Field::Number(u32::from(request.kind)),
            Field::Number(0),
            Field::Number(data.len() as u32),
            Field::Blob(data),
        ],
    )
}

#[derive(Default)]
struct Session {
    requester: Option<u32>,
    last_transaction: Option<u32>,
    // Menu selection belongs to this TCP client and menu location, never global.
    menus: HashMap<u8, Vec<MenuItem>>,
}

impl Session {
    fn handle(&mut self, request: &Message, library: &LibrarySnapshot) -> io::Result<Vec<Message>> {
        // S05/S20 empty type1 packets repeat a pending transaction ID and
        // receive no separate response. Do not insert an unsolicited error.
        if request.kind == 1 && request.args.is_empty() && self.last_transaction == Some(request.tx)
        {
            return Ok(Vec::new());
        }
        self.last_transaction = Some(request.tx);
        if request.kind == 0 {
            let requester = argument(request, 0)?;
            if !(1..=6).contains(&requester) || request.args.len() != 1 {
                return Ok(vec![error(request)]);
            }
            self.requester = Some(requester);
            self.menus.clear();
            return Ok(vec![ack(request, SOURCE_NUMBER)]);
        }
        if request.kind == 0x0100 {
            return Ok(Vec::new());
        }
        // Rekordbox-source mount/media initialization. The response interface
        // (4000, [3007, 0]) is independently described by vynulldev/vynull's
        // dbserver handler. No implementation code was copied. Unlike the
        // hardware-captured fixtures below, this exchange is reference-derived
        // and still needs a capture from a CDJ talking to this server.
        if request.kind == 0x3007 && self.requester.is_some() {
            return Ok(vec![ack(request, 0)]);
        }
        let descriptor = argument(request, 0)?;
        if self.requester != Some(descriptor >> 24) {
            return Ok(vec![error(request)]);
        }
        let location = ((descriptor >> 16) & 0xff) as u8;
        // S20 cursor-selection notifications get an empty success and do not
        // replace the menu being paged.
        if request.kind == 0x3100
            && request.args.len() == 4
            && request
                .args
                .iter()
                .all(|arg| matches!(arg, Field::Number(_)))
        {
            return Ok(vec![ack(request, 0)]);
        }
        if request.kind == 0x3000 {
            let offset = argument(request, 1)? as usize;
            let limit = argument(request, 2)? as usize;
            if limit > MAX_PAGE {
                return Ok(vec![error(request)]);
            }
            let Some(items) = self.menus.get(&location) else {
                return Ok(vec![error(request)]);
            };
            let mut replies = vec![response(
                request,
                0x4001,
                vec![Field::Number(1), Field::Number(0)],
            )];
            replies.extend(
                items
                    .iter()
                    .skip(offset)
                    .take(limit)
                    .map(|item| item.message(request)),
            );
            replies.push(response(request, 0x4201, Vec::new()));
            return Ok(replies);
        }
        if matches!(
            request.kind,
            0x2003 | 0x2004 | 0x2104 | 0x2204 | 0x2904 | 0x2b04 | 0x2c04
        ) {
            let id = argument(request, if request.kind == 0x2004 { 2 } else { 1 })?;
            let track = library.tracks.iter().find(|track| track.id == id);
            let data = match (request.kind, track) {
                (0x2204, Some(track)) => beat_grid(track),
                (0x2004, Some(track)) if track.waveform_preview.len() == 900 => {
                    track.waveform_preview.clone()
                }
                (0x2904, Some(track)) if track.waveform_detail.len() <= MAX_BLOB_BYTES => {
                    track.waveform_detail.clone()
                }
                (0x2104 | 0x2b04, Some(track)) => cues(track, request.kind == 0x2b04).0,
                _ => Vec::new(),
            };
            let kind = match request.kind {
                0x2003 => 0x4002,
                0x2004 => 0x4402,
                0x2104 => 0x4702,
                0x2204 => 0x4602,
                0x2904 => 0x4a02,
                0x2b04 => 0x4e02,
                _ => 0x4f02,
            };
            let mut reply = binary(request, kind, data);
            if request.kind == 0x2104 {
                let (_, hot, memory) = track.map(|track| cues(track, false)).unwrap_or_default();
                reply.args.extend([
                    Field::Number(0x24),
                    Field::Number(hot),
                    Field::Number(memory),
                    Field::Number(0),
                    Field::Blob(Vec::new()),
                ]);
            } else if request.kind == 0x2b04 {
                let (_, hot, memory) = track.map(|track| cues(track, true)).unwrap_or_default();
                reply.args.push(Field::Number(hot + memory));
            } else if request.kind == 0x2c04 || request.kind == 0x2204 {
                reply.args.push(Field::Number(0));
            }
            return Ok(vec![reply]);
        }
        match menu(request, library) {
            Ok(Some(items)) => {
                if !self.menus.contains_key(&location) && self.menus.len() >= MAX_MENU_CONTEXTS {
                    return Ok(vec![error(request)]);
                }
                let count = items.len() as u32;
                self.menus.insert(location, items);
                Ok(vec![ack(request, count)])
            }
            Ok(None) => {
                self.menus.remove(&location);
                Ok(vec![ack(request, u32::MAX)])
            }
            Err(_) => Ok(vec![error(request)]),
        }
    }
}

// Retain partially read fields across short socket timeouts, while allowing
// shutdown to interrupt idle clients. A peer cannot hold an incomplete message
// open forever. The deadline starts when its first byte arrives.
struct ClientReader<'a> {
    stream: &'a mut TcpStream,
    stop: &'a AtomicBool,
    started: Option<Instant>,
}

impl Read for ClientReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.stop.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::ConnectionAborted,
                    "server stopped",
                ));
            }
            if self
                .started
                .is_some_and(|start| start.elapsed() > Duration::from_secs(10))
            {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "incomplete database request",
                ));
            }
            match self.stream.read(bytes) {
                Ok(count) => {
                    self.started.get_or_insert_with(Instant::now);
                    return Ok(count);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                other => return other,
            }
        }
    }
}

fn serve_client(
    mut stream: TcpStream,
    library: &RwLock<LibrarySnapshot>,
    stop: &AtomicBool,
) -> io::Result<()> {
    // BSD/macOS may inherit O_NONBLOCK from the listening socket.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.set_nodelay(true)?;
    let mut hello = [0; 5];
    ClientReader {
        stream: &mut stream,
        stop,
        started: Some(Instant::now()),
    }
    .read_exact(&mut hello)?;
    if hello != [0x11, 0, 0, 0, 1] {
        return Err(invalid("invalid database handshake"));
    }
    stream.write_all(&hello)?;
    let mut session = Session::default();
    while !stop.load(Ordering::Acquire) {
        let request = read_message(&mut ClientReader {
            stream: &mut stream,
            stop,
            started: None,
        })?;
        if request.kind == 0x0100 {
            return Ok(());
        }
        let replies = {
            let snapshot = library
                .read()
                .map_err(|_| io::Error::other("library lock poisoned"))?;
            session
                .handle(&request, &snapshot)
                .unwrap_or_else(|_| vec![error(&request)])
        };
        for reply in replies {
            stream.write_all(&encode_message(&reply))?;
        }
    }
    Ok(())
}

/// Bind both listeners before spawning threads, so partial startup cannot leave
/// a database service running. The caller owns the stop flag and joins handles.
pub fn start_dbserver(
    bind_ip: Ipv4Addr,
    query_port: u16,
    database_port: u16,
    library: Arc<RwLock<LibrarySnapshot>>,
    stop: Arc<AtomicBool>,
) -> io::Result<Vec<JoinHandle<()>>> {
    let query = TcpListener::bind((bind_ip, query_port))?;
    let database = TcpListener::bind((bind_ip, database_port))?;
    start_listeners(query, database, library, stop)
}

fn start_listeners(
    query: TcpListener,
    database: TcpListener,
    library: Arc<RwLock<LibrarySnapshot>>,
    stop: Arc<AtomicBool>,
) -> io::Result<Vec<JoinHandle<()>>> {
    query.set_nonblocking(true)?;
    database.set_nonblocking(true)?;
    let port = database.local_addr()?.port();
    let query_stop = Arc::clone(&stop);
    let query_thread = thread::spawn(move || {
        while !query_stop.load(Ordering::Acquire) {
            match query.accept() {
                Ok((mut stream, _)) => {
                    if stream.set_nonblocking(false).is_err() {
                        continue;
                    }
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
                    let mut bytes = [0; 19];
                    if stream.read_exact(&mut bytes).is_ok() && bytes == QUERY {
                        let _ = stream.write_all(&port.to_be_bytes());
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20))
                }
                Err(_) => break,
            }
        }
    });
    let database_thread = thread::spawn(move || {
        let mut clients: Vec<JoinHandle<()>> = Vec::new();
        while !stop.load(Ordering::Acquire) {
            let mut index = 0;
            while index < clients.len() {
                if clients[index].is_finished() {
                    let _ = clients.swap_remove(index).join();
                } else {
                    index += 1;
                }
            }
            match database.accept() {
                Ok((stream, _)) if clients.len() < MAX_CLIENTS => {
                    let library = Arc::clone(&library);
                    let stop = Arc::clone(&stop);
                    clients.push(thread::spawn(move || {
                        let _ = serve_client(stream, &library, &stop);
                    }));
                }
                Ok(_) => {} // Drop excess connections instead of spawning unbounded threads.
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20))
                }
                Err(_) => break,
            }
        }
        for client in clients {
            let _ = client.join();
        }
    });
    Ok(vec![query_thread, database_thread])
}

/// Encode real three-band RMS analysis into the legacy blue waveform format.
/// Inputs must have one sample per 1/150 second of audio. Returns empty data if
/// analysis is missing or the lengths disagree. No fabricated fallback waveform.
pub fn encode_three_band_waveform(low: &[f32], mid: &[f32], high: &[f32]) -> (Vec<u8>, Vec<u8>) {
    if low.is_empty()
        || low.len() != mid.len()
        || low.len() != high.len()
        || low.len() > MAX_BLOB_BYTES
    {
        return (Vec::new(), Vec::new());
    }
    let clean = |value: f32| {
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    let detail: Vec<u8> = low
        .iter()
        .zip(mid)
        .zip(high)
        .map(|((&low, &mid), &high)| {
            let (low, mid, high) = (clean(low), clean(mid), clean(high));
            let amplitude = (low * low + mid * mid + high * high).sqrt().min(1.0);
            let height = (amplitude.sqrt() * 31.0).round() as u8;
            let whiteness = if low + mid + high > 0.0 {
                (high / (low + mid + high) * 7.0).round() as u8
            } else {
                0
            };
            height | (whiteness << 5)
        })
        .collect();
    let mut preview = vec![0; 900];
    for column in 0..400 {
        let start = column * detail.len() / 400;
        let end = ((column + 1) * detail.len() / 400)
            .max(start + 1)
            .min(detail.len());
        let value = detail[start..end]
            .iter()
            .max_by_key(|value| **value & 31)
            .copied()
            .unwrap_or(0);
        preview[column * 2] = value & 31;
        preview[column * 2 + 1] = value >> 5;
    }
    for column in 0..100 {
        preview[800 + column] = (0..4)
            .map(|part| preview[(column * 4 + part) * 2] / 2)
            .max()
            .unwrap_or(0);
    }
    (preview, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LinkBeat, LinkCue, LinkPlaylist};
    use std::io::Cursor;

    // Literal bytes from real CDJ-2000nexus firmware1.44 traffic, recorded
    // 2026-07-29 by the independent dysentery hardware-capture contributor.
    // These fixtures are not generated by this server or its encoder.
    // https://github.com/Deep-Symmetry/dysentery/tree/main/doc/assets/captures
    fn capture(hex: &str) -> Vec<u8> {
        hex.split_whitespace()
            .collect::<String>()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn captured_s05_root_category_has_wire_id_and_localization_markers() {
        // S05 source port1058 -> 1051, TX038001a8 and render TX038001a9.
        let query = read_message(&mut Cursor::new(capture("11872349ae11038001a81010000f03140000000c060606000000000000000000110101030111000000001100ffffff"))).unwrap();
        let render = read_message(&mut Cursor::new(capture("11872349ae11038001a91030000f06140000000c0606060606060000000000001101010301110000000011000000061100000000110000000c1100000000"))).unwrap();
        let expected = capture("11872349ae11038001a91041010f0c140000000c060606020602060606060606110000000011000000051100000016260000000bfffa0050004c00410059004c004900530054fffb0000110000000226000000010000110000008411000000001100000000110000000011000000001100000000");
        let mut session = session();
        session.handle(&query, &library()).unwrap();
        let result = session.handle(&render, &library()).unwrap();
        let playlist = result
            .iter()
            .find(|message| message.kind == 0x4101 && message.args[6] == Field::Number(0x84))
            .unwrap();
        assert_eq!(encode_message(playlist), expected);
        assert_eq!(read_message(&mut Cursor::new(expected)).unwrap(), *playlist);
    }

    #[test]
    fn captured_s05_track_entry_flags_are_preserved() {
        // S05 render038001b7, first playlist entry. Artwork is unavailable in
        // our snapshot model, so only that captured field intentionally differs.
        let raw = capture("11872349ae11038001b71041010f0c140000000c060606020602060606060606110000001711000000131100000016260000000b0043006f0070006c0061006e00640020004f00530000110000002626000000130057004100530045004900200022004a004a00220020004300480049004b004100440041000011000007041101000000110000000f110000000111000001001100000000");
        let mut expected = read_message(&mut Cursor::new(raw)).unwrap();
        expected.args[8] = Field::Number(0);
        let track = LinkTrack {
            id: 19,
            title: "Copland OS".into(),
            artist: "WASEI \"JJ\" CHIKADA".into(),
            ..LinkTrack::default()
        };
        let artists = BTreeMap::from([(track.artist.clone(), 23)]);
        let mut item = track_item(&track, &artists);
        item.position = 1;
        let request = Message {
            tx: 0x0380_01b7,
            kind: 0x3000,
            args: vec![],
        };
        assert_eq!(item.message(&request), expected);
    }

    #[test]
    fn captured_s13_track_info_uses_format_and_file_size() {
        // S13 format ground truth: track614 MP3,425s,145BPM,6911124 bytes.
        let query = read_message(&mut Cursor::new(capture(
            "11872349ae110380000e1021020f02140000000c06060000000000000000000011020803011100000266",
        )))
        .unwrap();
        let expected_ack = capture(
            "11872349ae110380000e1040000f02140000000c06060000000000000000000011000021021100000006",
        );
        let render = read_message(&mut Cursor::new(capture("11872349ae110380000f1030000f06140000000c060606060606000000000000110208030111000000001100000006110000000011000000061100000000"))).unwrap();
        let captured_format = capture("11872349ae110380000f1041010f0c140000000c06060602060206060606060611000000001100000001110000000226000000010000110000000226000000010000110000000411000000001100000000110000000011000000001100000000");
        let captured_path = read_message(&mut Cursor::new(capture("11872349ae110380000f1041010f0c140000000c060606020602060606060606110069749411000002661100000074260000003a002f0043006f006e00740065006e00740073002f0046004f0052004d0041005400200054004500530054002f005400720069004d0069007800780078002f003000330020004d005000330020004d00500045004700310020003100320038006b002000340034006b0031002e006d007000330000110000000226000000010000110000000011000000001100000000110000000011000000001100000000"))).unwrap();
        let mut library = library();
        let track = &mut library.tracks[0];
        track.id = 614;
        track.duration_ms = 425_000;
        track.bpm = 145.0;
        track.byte_size = 6_911_124;
        track.path = "/registered/test.mp3".into();
        let mut session = Session {
            requester: Some(2),
            ..Session::default()
        };
        let response = session.handle(&query, &library).unwrap();
        assert_eq!(encode_message(&response[0]), expected_ack);
        let result = session.handle(&render, &library).unwrap();
        assert_eq!(encode_message(&result[1]), captured_format);
        let path = &result[5];
        assert_eq!(path.args[0], captured_path.args[0]);
        assert_eq!(path.args[1], captured_path.args[1]);
        assert_eq!(path.args[3], Field::Text("/conduction/614/test.mp3".into()));
        assert_eq!(
            result
                .iter()
                .filter(|message| message.kind == 0x4101)
                .map(|message| message.args[6].number().unwrap())
                .collect::<Vec<_>>(),
            [4, 11, 13, 35, 0, 47]
        );
    }

    #[test]
    fn captured_s13_beat_and_empty_cue_wire_envelopes() {
        let mut library = library();
        library.tracks[0].id = 614;
        library.tracks[0].cues.clear();
        library.tracks[0].beats = vec![LinkBeat {
            time_ms: 47,
            beat: 1,
            bpm: 145.0,
        }];
        let mut session = Session {
            requester: Some(2),
            ..Session::default()
        };
        let cue = read_message(&mut Cursor::new(capture(
            "11872349ae11038000131021040f02140000000c06060000000000000000000011020803011100000266",
        )))
        .unwrap();
        let expected = capture("11872349ae11038000131047020f09140000000c0606060306060606030000001100002104110000000011000000001100000024110000000011000000001100000000");
        assert_eq!(
            encode_message(&session.handle(&cue, &library).unwrap()[0]),
            expected
        );
        let beat = read_message(&mut Cursor::new(capture(
            "11872349ae110380001c1022040f02140000000c06060000000000000000000011020803011100000266",
        )))
        .unwrap();
        let reply = &session.handle(&beat, &library).unwrap()[0];
        // Captured envelope includes a fifth numeric argument (omitted in the
        // older prose description), and captured entry padding is ffffffff.
        let expected_prefix = capture(
            "11872349ae110380001c1046020f05140000000c06060603060000000000000011000022041100000000",
        );
        assert_eq!(
            &encode_message(reply)[..expected_prefix.len()],
            expected_prefix
        );
        let Field::Blob(data) = &reply.args[3] else {
            panic!("missing beats")
        };
        assert_eq!(&data[20..36], capture("0100a4382f000000ffffffffffffffff"));
        assert_eq!(&data[..16], capture("00000800010000001000000001000000"));
        assert_eq!(reply.args[4], Field::Number(0));
    }

    #[test]
    fn reference_derived_media_init_and_captured_pending_query_notice() {
        let mut session = session();
        let library = library();
        // Reference-derived, not claimed to be a captured hardware fixture.
        let init = request(0x3007, &[]);
        assert_eq!(session.handle(&init, &library).unwrap()[0], ack(&init, 0));
        // S05's empty type1 repeats the preceding render's transaction.
        let query = request(0x1004, &[0x0101_0301, 0]);
        session.handle(&query, &library).unwrap();
        assert!(session
            .handle(&request(1, &[]), &library)
            .unwrap()
            .is_empty());
        let cursor = request(0x3100, &[0x0101_0301, 41, 0, 0]);
        assert_eq!(
            session.handle(&cursor, &library).unwrap()[0],
            ack(&cursor, 0)
        );
        let result = session
            .handle(&request(0x3000, &[0x0101_0301, 0, 6, 0, 1, 0]), &library)
            .unwrap();
        assert_eq!(result.len(), 3);
    }

    fn library() -> LibrarySnapshot {
        LibrarySnapshot {
            tracks: vec![LinkTrack {
                id: 41,
                title: "夜の曲 🎵".into(),
                artist: "東京".into(),
                album: "Album".into(),
                genre: "House".into(),
                duration_ms: 10_000,
                bpm: 120.0,
                path: "/private/audio/music.wav".into(),
                byte_size: 100,
                beats: vec![LinkBeat {
                    time_ms: 500,
                    beat: 2,
                    bpm: 120.0,
                }],
                cues: vec![
                    LinkCue {
                        slot: 1,
                        time_ms: 1000,
                        end_ms: Some(2000),
                        name: "入口".into(),
                    },
                    LinkCue {
                        slot: 8,
                        time_ms: 3000,
                        end_ms: None,
                        name: "出口".into(),
                    },
                ],
                waveform_preview: vec![7; 900],
                waveform_detail: vec![9; 1500],
            }],
            playlists: vec![LinkPlaylist {
                id: 7,
                name: "Set".into(),
                track_ids: vec![41],
            }],
        }
    }

    fn request(kind: u16, numbers: &[u32]) -> Message {
        Message {
            tx: 7,
            kind,
            args: numbers.iter().map(|&n| Field::Number(n)).collect(),
        }
    }

    fn session() -> Session {
        Session {
            requester: Some(1),
            ..Session::default()
        }
    }

    #[test]
    fn wire_roundtrip_handles_unicode_and_omitted_blob() {
        let message = Message {
            tx: 8,
            kind: 0x1300,
            args: vec![
                Field::Number(4),
                Field::Text("夜🎵".into()),
                Field::Number(0),
                Field::Blob(vec![]),
            ],
        };
        let encoded = encode_message(&message);
        assert_eq!(read_message(&mut Cursor::new(&encoded)).unwrap(), message);
        for count in 0..encoded.len() {
            assert!(read_message(&mut Cursor::new(&encoded[..count])).is_err());
        }
    }

    #[test]
    fn rejects_unbounded_fields_and_bad_tags() {
        let mut budget = MAX_REQUEST_BYTES;
        assert!(read_field(
            &mut Cursor::new([0x14, 0xff, 0xff, 0xff, 0xff]),
            &mut budget
        )
        .is_err());
        let mut bytes = encode_message(&request(0x1004, &[0x0101_0401, 0]));
        bytes[20] = 2;
        assert!(read_message(&mut Cursor::new(bytes)).is_err());
    }

    #[test]
    fn menus_are_private_and_search_preserves_japanese() {
        let library = library();
        let mut first = session();
        let mut second = session();
        first
            .handle(&request(0x1004, &[0x0101_0401, 0]), &library)
            .unwrap();
        second
            .handle(&request(0x1105, &[0x0101_0401, 0, 0, 1]), &library)
            .unwrap();
        let render = request(0x3000, &[0x0101_0401, 0, 64, 0, 64, 0]);
        let a = first.handle(&render, &library).unwrap();
        let b = second.handle(&render, &library).unwrap();
        assert_eq!(a[1].args[3], Field::Text("夜の曲 🎵".into()));
        assert_eq!(b[1].args[3], Field::Text("Set".into()));
        let search = Message {
            tx: 9,
            kind: 0x1300,
            args: vec![
                Field::Number(0x0101_0401),
                Field::Number(0),
                Field::Number(4),
                Field::Text("夜".into()),
                Field::Number(0),
            ],
        };
        assert_eq!(
            first.handle(&search, &library).unwrap()[0].args[1],
            Field::Number(1)
        );
        assert_eq!(
            first
                .handle(&request(0x3000, &[0x0101_0401, u32::MAX, 64]), &library)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn loading_metadata_exposes_only_exported_path() {
        let library = library();
        let mut session = session();
        session
            .handle(&request(0x2102, &[0x0102_0401, 41]), &library)
            .unwrap();
        let replies = session
            .handle(&request(0x3000, &[0x0102_0401, 0, 64]), &library)
            .unwrap();
        let path = replies
            .iter()
            .find(|reply| reply.kind == 0x4101 && reply.args[6] == Field::Number(0))
            .unwrap();
        assert_eq!(path.args[3], Field::Text("/conduction/41/music.wav".into()));
        assert!(!format!("{replies:?}").contains("/private"));
    }

    #[test]
    fn beat_grid_and_cues_use_correct_units_and_endian() {
        let library = library();
        let track = &library.tracks[0];
        let beats = beat_grid(track);
        assert_eq!(&beats[20..28], &[2, 0, 0xe0, 0x2e, 0xf4, 1, 0, 0]);
        let (old, hot, memory) = cues(track, false);
        assert_eq!((hot, memory, old.len()), (1, 0, 36));
        assert_eq!(&old[12..16], &150u32.to_le_bytes());
        assert_eq!(&old[16..20], &300u32.to_le_bytes());
        let (extended, hot, _) = cues(track, true);
        assert_eq!(hot, 2);
        let next = u32::from_le_bytes(extended[..4].try_into().unwrap()) as usize;
        assert_eq!(extended[next + 4], 8);
        assert_eq!(extended[0x4a..0x4c], ('入' as u16).to_le_bytes());
    }

    #[test]
    fn generated_waveform_is_bounded_and_absent_without_analysis() {
        let (preview, detail) = encode_three_band_waveform(&[0.0, 0.5, 1.0], &[0.0; 3], &[0.0; 3]);
        assert_eq!(preview.len(), 900);
        assert_eq!(detail.len(), 3);
        assert_eq!(detail[0], 0);
        assert!(detail[1] > 0 && detail[2] == 31);
        assert!(encode_three_band_waveform(&[], &[], &[]).0.is_empty());
    }

    #[test]
    fn loopback_query_handshake_and_metadata_roundtrip() {
        let query = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let database = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let query_address = query.local_addr().unwrap();
        let database_address = database.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let handles = start_listeners(
            query,
            database,
            Arc::new(RwLock::new(library())),
            Arc::clone(&stop),
        )
        .unwrap();
        let mut query = TcpStream::connect(query_address).unwrap();
        query
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        query.write_all(QUERY).unwrap();
        let mut port = [0; 2];
        query.read_exact(&mut port).unwrap();
        assert_eq!(u16::from_be_bytes(port), database_address.port());
        let mut client = TcpStream::connect(database_address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client.write_all(&[0x11, 0, 0, 0, 1]).unwrap();
        let mut hello = [0; 5];
        client.read_exact(&mut hello).unwrap();
        assert_eq!(hello, [0x11, 0, 0, 0, 1]);
        client
            .write_all(&encode_message(&request(0, &[1])))
            .unwrap();
        assert_eq!(
            read_message(&mut client).unwrap().args[1],
            Field::Number(SOURCE_NUMBER)
        );
        client
            .write_all(&encode_message(&request(0x2102, &[0x0101_0401, 41])))
            .unwrap();
        assert_eq!(read_message(&mut client).unwrap().args[1], Field::Number(6));
        client
            .write_all(&encode_message(&request(0x3000, &[0x0101_0401, 0, 64])))
            .unwrap();
        assert_eq!(read_message(&mut client).unwrap().kind, 0x4001);
        for _ in 0..6 {
            assert_eq!(read_message(&mut client).unwrap().kind, 0x4101);
        }
        assert_eq!(read_message(&mut client).unwrap().kind, 0x4201);
        // Stop also joins an idle client instead of waiting for it to disconnect.
        stop.store(true, Ordering::Release);
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn loopback_database_path_loads_through_concurrent_nfs_service() {
        use std::fs::File;
        use std::net::UdpSocket;
        fn opaque(bytes: &[u8]) -> Vec<u8> {
            let mut value = (bytes.len() as u32).to_be_bytes().to_vec();
            value.extend(bytes);
            value.resize(value.len().div_ceil(4) * 4, 0);
            value
        }
        fn rpc(
            socket: &UdpSocket,
            program: u32,
            version: u32,
            procedure: u32,
            args: &[u8],
        ) -> Vec<u8> {
            let mut bytes: Vec<u8> = [9u32, 0, 2, program, version, procedure, 0, 0, 0, 0]
                .into_iter()
                .flat_map(u32::to_be_bytes)
                .collect();
            bytes.extend(args);
            socket.send(&bytes).unwrap();
            let mut reply = [0; 16384];
            let count = socket.recv(&mut reply).unwrap();
            assert_eq!(&reply[..4], &9u32.to_be_bytes());
            assert_eq!(&reply[20..24], &[0; 4]);
            reply[..count].to_vec()
        }
        let mut snapshot = library();
        snapshot.tracks[0].path = std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        snapshot.tracks[0].byte_size = snapshot.tracks[0].path.metadata().unwrap().len();
        let mut expected = [0; 64];
        File::open(&snapshot.tracks[0].path)
            .unwrap()
            .read_exact(&mut expected)
            .unwrap();
        let library = Arc::new(RwLock::new(snapshot));
        let stop = Arc::new(AtomicBool::new(false));
        let nfs = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let nfs_address = nfs.local_addr().unwrap();
        let nfs_worker =
            crate::nfs::start_socket(nfs, Arc::clone(&library), Arc::clone(&stop)).unwrap();
        let database = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let database_address = database.local_addr().unwrap();
        let query = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let workers = start_listeners(query, database, library, Arc::clone(&stop)).unwrap();
        let mut client = TcpStream::connect(database_address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client.write_all(&[0x11, 0, 0, 0, 1]).unwrap();
        let mut hello = [0; 5];
        client.read_exact(&mut hello).unwrap();
        client
            .write_all(&encode_message(&request(0, &[1])))
            .unwrap();
        read_message(&mut client).unwrap();
        client
            .write_all(&encode_message(&request(0x2102, &[0x0101_0401, 41])))
            .unwrap();
        assert_eq!(read_message(&mut client).unwrap().args[1], Field::Number(6));
        client
            .write_all(&encode_message(&request(0x3000, &[0x0101_0401, 0, 64])))
            .unwrap();
        let mut path = String::new();
        loop {
            let message = read_message(&mut client).unwrap();
            if message.kind == 0x4201 {
                break;
            }
            if message.kind == 0x4101 && message.args[6] == Field::Number(0) {
                let Field::Text(value) = &message.args[3] else {
                    panic!("missing path")
                };
                path = value.clone();
            }
        }
        assert!(path.starts_with("/conduction/41/"));
        let udp = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        udp.connect(nfs_address).unwrap();
        udp.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let get_port: Vec<u8> = [100_003u32, 2, 17, 0]
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect();
        let mapped = rpc(&udp, 100_000, 2, 3, &get_port);
        assert_eq!(
            &mapped[24..28],
            &u32::from(nfs_address.port()).to_be_bytes()
        );
        let mounted = rpc(&udp, 100_005, 1, 1, &opaque(b"/"));
        assert_eq!(&mounted[24..28], &[0; 4]);
        let mut handle = mounted[28..60].to_vec();
        for component in path.split('/').filter(|component| !component.is_empty()) {
            let mut args = handle;
            args.extend(opaque(component.as_bytes()));
            let looked_up = rpc(&udp, 100_003, 2, 4, &args);
            assert_eq!(&looked_up[24..28], &[0; 4]);
            handle = looked_up[28..60].to_vec();
        }
        let mut args = handle;
        args.extend([0u32, 64, 64].into_iter().flat_map(u32::to_be_bytes));
        let data = rpc(&udp, 100_003, 2, 6, &args);
        assert_eq!(&data[24..28], &[0; 4]);
        assert_eq!(&data[96..100], &64u32.to_be_bytes());
        assert_eq!(&data[100..164], &expected);
        // The same persistent db connection still serves beat data during loading.
        client
            .write_all(&encode_message(&request(0x2204, &[0x0108_0401, 41])))
            .unwrap();
        assert_eq!(read_message(&mut client).unwrap().kind, 0x4602);
        stop.store(true, Ordering::Release);
        for worker in workers {
            worker.join().unwrap();
        }
        nfs_worker.join().unwrap();
    }
}
