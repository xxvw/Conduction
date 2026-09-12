//! Conduction Library ↔ rekordbox XML mapping.
//!
//! Phase 2 implements the **outgoing** direction only: a `Library` snapshot
//! becomes a [`DjPlaylists`] tree we can serialize to disk. Track IDs in
//! rekordbox are `u32`, so the mapping assigns them positionally
//! (1..=N over the library's `list_tracks()` order) since Conduction's
//! own `TrackId` is a UUID and the XML is one-way at this stage.

use std::path::Path;

use conduction_core::Track as CoreTrack;
use conduction_library::Library;

use crate::xml::{Collection, DjPlaylists, Node, Playlists, PositionMark, Product, Tempo, Track};

/// PRODUCT block written to every export.
fn product_block() -> Product {
    Product {
        name: "Conduction".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        company: "conduction".to_string(),
    }
}

/// Encode an absolute filesystem path as a `file://` URI rekordbox accepts.
fn path_to_location_uri(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.starts_with('/') {
        format!("file://localhost{}", s)
    } else {
        format!("file://localhost/{}", s)
    }
}

/// PLAYLISTS block with an empty ROOT folder (the mapping for Conduction
/// setlists is wired in a later phase).
fn empty_playlists_block() -> Playlists {
    Playlists {
        root: Some(Node {
            name: "ROOT".to_string(),
            kind: 0,
            count: Some(0),
            ..Default::default()
        }),
    }
}

/// Full Library → `DjPlaylists` mapping.
///
/// Intentionally consumes a `&mut Library` (rusqlite requires it for any
/// query) but the call is read-only.
pub fn library_to_dj_playlists(
    library: &mut Library,
) -> Result<DjPlaylists, conduction_library::LibraryError> {
    let core_tracks = library.list_tracks()?;
    let mut xml_tracks: Vec<Track> = Vec::with_capacity(core_tracks.len());
    for (idx, t) in core_tracks.iter().enumerate() {
        let track_id = (idx + 1) as u32;
        let beats = library.load_beatgrid(t.id)?;
        let hot_cues = library.list_hot_cues(t.id)?;
        let cues = library.list_cues_for_track(t.id)?;
        xml_tracks.push(map_track(t, track_id, &beats, &hot_cues, &cues));
    }

    Ok(DjPlaylists {
        version: "1.0.0".to_string(),
        product: Some(product_block()),
        collection: Some(Collection {
            entries: xml_tracks.len() as u32,
            tracks: xml_tracks,
        }),
        playlists: Some(empty_playlists_block()),
    })
}

/// One Conduction Track + analysis → one rekordbox `<TRACK>` row.
///
/// Beatgrid is collapsed to a single TEMPO anchor at the first beat
/// (Phase 2 assumes constant-tempo tracks; variable tempo support lands
/// alongside hot-cue type mapping refinements later).
fn map_track(
    track: &CoreTrack,
    rekordbox_track_id: u32,
    beats: &[conduction_core::Beat],
    hot_cues: &[(u8, f64)],
    cues: &[conduction_core::Cue],
) -> Track {
    let first_beat_sec = beats.first().map(|b| b.position_sec).unwrap_or(0.0);
    let bpm = track.bpm as f64;
    let tempos = if bpm > 0.0 {
        vec![Tempo {
            inizio: first_beat_sec,
            bpm,
            metro: "4/4".to_string(),
            battito: 1,
        }]
    } else {
        vec![]
    };

    let mut position_marks: Vec<PositionMark> = Vec::with_capacity(hot_cues.len() + cues.len());
    for (slot, sec) in hot_cues {
        // Conduction's hot-cue slot is 1..=8; rekordbox's Num is 0..=7.
        let num = i32::from(*slot).saturating_sub(1);
        position_marks.push(PositionMark {
            name: String::new(),
            kind: 0,
            start: *sec,
            end: None,
            num,
        });
    }
    // typed Cues from conduction-core become memory cues (Num = -1).
    // Position is in beats; convert via the first TEMPO's BPM.
    for c in cues {
        let sec = if bpm > 0.0 {
            (c.position_beats * 60.0 / bpm) + first_beat_sec
        } else {
            c.position_beats
        };
        position_marks.push(PositionMark {
            name: cue_name(&c.cue_type),
            kind: 0,
            start: sec,
            end: None,
            num: -1,
        });
    }

    Track {
        track_id: Some(rekordbox_track_id),
        name: empty_to_none(&track.title),
        artist: empty_to_none(&track.artist),
        album: empty_to_none(&track.album),
        genre: empty_to_none(&track.genre),
        total_time: Some(track.duration.as_secs() as u32),
        average_bpm: if bpm > 0.0 { Some(bpm) } else { None },
        tonality: Some(track.key.to_camelot()),
        location: Some(path_to_location_uri(&track.path)),
        tempos,
        position_marks,
        ..Default::default()
    }
}

fn empty_to_none(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn cue_name(kind: &conduction_core::CueType) -> String {
    use conduction_core::CueType::*;
    match kind {
        Drop => "Drop".into(),
        IntroStart => "Intro".into(),
        IntroEnd => "Intro End".into(),
        Breakdown => "Breakdown".into(),
        Outro => "Outro".into(),
        HotCue => "Hot Cue".into(),
        CustomHotCue => "Custom".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_location_uri_keeps_absolute_paths() {
        let uri = path_to_location_uri(Path::new("/Users/me/Music/a.mp3"));
        assert_eq!(uri, "file://localhost/Users/me/Music/a.mp3");
    }

    #[test]
    fn path_to_location_uri_inserts_slash_for_relative() {
        let uri = path_to_location_uri(Path::new("Music/a.mp3"));
        assert_eq!(uri, "file://localhost/Music/a.mp3");
    }

    #[test]
    fn empty_strings_become_none() {
        assert_eq!(empty_to_none(""), None);
        assert_eq!(empty_to_none("Title"), Some("Title".to_string()));
    }
}
