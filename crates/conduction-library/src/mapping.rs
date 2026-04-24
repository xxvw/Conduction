use std::collections::BTreeSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use conduction_core::{Cue, CueId, CueType, Key, KeyMode, MixRole, Track, TrackId};
use rusqlite::Row;
use uuid::Uuid;

use crate::error::{LibraryError, LibraryResult};

// --- Key mode ---

pub fn key_mode_to_i64(mode: KeyMode) -> i64 {
    match mode {
        KeyMode::Major => 0,
        KeyMode::Minor => 1,
    }
}

pub fn key_mode_from_i64(value: i64) -> LibraryResult<KeyMode> {
    match value {
        0 => Ok(KeyMode::Major),
        1 => Ok(KeyMode::Minor),
        other => Err(LibraryError::Unsupported(format!("key_mode={other}"))),
    }
}

// --- CueType ---

pub fn cue_type_to_str(ty: CueType) -> &'static str {
    match ty {
        CueType::HotCue => "HotCue",
        CueType::IntroStart => "IntroStart",
        CueType::IntroEnd => "IntroEnd",
        CueType::Breakdown => "Breakdown",
        CueType::Drop => "Drop",
        CueType::Outro => "Outro",
        CueType::CustomHotCue => "CustomHotCue",
    }
}

pub fn cue_type_from_str(s: &str) -> LibraryResult<CueType> {
    match s {
        "HotCue" => Ok(CueType::HotCue),
        "IntroStart" => Ok(CueType::IntroStart),
        "IntroEnd" => Ok(CueType::IntroEnd),
        "Breakdown" => Ok(CueType::Breakdown),
        "Drop" => Ok(CueType::Drop),
        "Outro" => Ok(CueType::Outro),
        "CustomHotCue" => Ok(CueType::CustomHotCue),
        other => Err(LibraryError::Unsupported(format!("cue_type={other}"))),
    }
}

// --- MixRole set <-> CSV ---

pub fn mix_roles_to_csv(roles: &BTreeSet<MixRole>) -> String {
    roles
        .iter()
        .map(|r| match r {
            MixRole::Entry => "Entry",
            MixRole::Exit => "Exit",
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub fn mix_roles_from_csv(s: &str) -> LibraryResult<BTreeSet<MixRole>> {
    let mut set = BTreeSet::new();
    for part in s.split(',').filter(|p| !p.is_empty()) {
        match part.trim() {
            "Entry" => {
                set.insert(MixRole::Entry);
            }
            "Exit" => {
                set.insert(MixRole::Exit);
            }
            other => return Err(LibraryError::Unsupported(format!("mix_role={other}"))),
        }
    }
    Ok(set)
}

// --- DateTime ---

pub fn dt_to_str(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

pub fn dt_from_str(s: &str) -> LibraryResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| LibraryError::Unsupported(format!("datetime={s} ({e})")))
}

// --- Row -> Track ---

pub fn track_from_row(row: &Row<'_>) -> LibraryResult<Track> {
    let id_str: String = row.get("id")?;
    let path_str: String = row.get("path")?;
    let duration_sec: f64 = row.get("duration_sec")?;
    let key_num: i64 = row.get("key_camelot_number")?;
    let key_mode_i: i64 = row.get("key_mode")?;
    let analyzed_at: Option<String> = row.get("analyzed_at")?;

    let id = parse_uuid(&id_str)?;
    let key = Key::new(
        key_num
            .try_into()
            .map_err(|_| LibraryError::Unsupported(format!("key_num out of range: {key_num}")))?,
        key_mode_from_i64(key_mode_i)?,
    )?;

    let analyzed_at = match analyzed_at {
        Some(s) => Some(dt_from_str(&s)?),
        None => None,
    };

    Ok(Track {
        id: TrackId::from_uuid(id),
        path: PathBuf::from(path_str),
        title: row.get("title")?,
        artist: row.get("artist")?,
        album: row.get("album")?,
        genre: row.get("genre")?,
        duration: Duration::from_secs_f64(duration_sec.max(0.0)),
        bpm: row.get::<_, f64>("bpm")? as f32,
        key,
        energy: row.get::<_, f64>("energy")? as f32,
        cues: Vec::new(),
        beatgrid: Vec::new(),
        beatgrid_verified: row.get::<_, i64>("beatgrid_verified")? != 0,
        analyzed_at,
    })
}

// --- Row -> Cue ---

pub fn cue_from_row(row: &Row<'_>) -> LibraryResult<Cue> {
    let id_str: String = row.get("id")?;
    let track_id_str: String = row.get("track_id")?;
    let section_start: Option<f64> = row.get("section_start")?;
    let section_end: Option<f64> = row.get("section_end")?;
    let key_num: i64 = row.get("key_camelot_number")?;
    let key_mode_i: i64 = row.get("key_mode")?;
    let mixable: String = row.get("mixable_as")?;
    let cue_type: String = row.get("cue_type")?;

    let key = Key::new(
        key_num
            .try_into()
            .map_err(|_| LibraryError::Unsupported(format!("key_num out of range: {key_num}")))?,
        key_mode_from_i64(key_mode_i)?,
    )?;

    let section = match (section_start, section_end) {
        (Some(s), Some(e)) if e > s => Some(s..e),
        _ => None,
    };

    Ok(Cue {
        id: CueId::from_uuid(parse_uuid(&id_str)?),
        track_id: TrackId::from_uuid(parse_uuid(&track_id_str)?),
        position_beats: row.get("position_beats")?,
        cue_type: cue_type_from_str(&cue_type)?,
        section,
        bpm_at_cue: row.get::<_, f64>("bpm_at_cue")? as f32,
        key_at_cue: key,
        energy_level: row.get::<_, f64>("energy_level")? as f32,
        phrase_length: row.get::<_, i64>("phrase_length")? as u32,
        mixable_as: mix_roles_from_csv(&mixable)?,
        compatible_energy: (row.get::<_, f64>("compatible_energy_start")? as f32)
            ..(row.get::<_, f64>("compatible_energy_end")? as f32),
    })
}

fn parse_uuid(s: &str) -> LibraryResult<Uuid> {
    Uuid::from_str(s).map_err(|e| LibraryError::Unsupported(format!("uuid={s} ({e})")))
}
