//! `<COLLECTION>` and its `<TRACK>` children, with nested `<TEMPO>` /
//! `<POSITION_MARK>` elements.
//!
//! rekordbox.app ships dozens of TRACK attributes across versions; this
//! model keeps the ones Conduction actively consumes (BPM, key, hot
//! cues, beatgrid) and accepts the rest verbatim by treating every
//! attribute as `Option<String>` so unknown values pass through.
//!
//! Every `Option<T>` attribute is annotated with
//! `skip_serializing_if = "Option::is_none"` so `None` values don't
//! emit empty `Attr=""`, which would otherwise fail round-tripping
//! through numeric types (`"" → u32` is an error).

use serde::{Deserialize, Serialize};

/// `<COLLECTION Entries="N"> <TRACK>... </COLLECTION>`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename = "COLLECTION")]
pub struct Collection {
    /// rekordbox.app writes this even though it's redundant with `tracks.len()`.
    #[serde(rename = "@Entries", default)]
    pub entries: u32,
    #[serde(rename = "TRACK", default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<Track>,
}

/// One `<TRACK>` row.
///
/// All metadata is `Option<String>` (or `Option<NumericType>`) so missing
/// attributes round-trip cleanly. The nested TEMPO / POSITION_MARK lists
/// default to empty.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename = "TRACK")]
pub struct Track {
    #[serde(rename = "@TrackID", default, skip_serializing_if = "Option::is_none")]
    pub track_id: Option<u32>,
    #[serde(rename = "@Name", default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "@Artist", default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "@Composer", default, skip_serializing_if = "Option::is_none")]
    pub composer: Option<String>,
    #[serde(rename = "@Album", default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(rename = "@Grouping", default, skip_serializing_if = "Option::is_none")]
    pub grouping: Option<String>,
    #[serde(rename = "@Genre", default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(rename = "@Kind", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(rename = "@Size", default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(rename = "@TotalTime", default, skip_serializing_if = "Option::is_none")]
    pub total_time: Option<u32>,
    #[serde(rename = "@DiscNumber", default, skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
    #[serde(rename = "@TrackNumber", default, skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u32>,
    #[serde(rename = "@Year", default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
    #[serde(rename = "@AverageBpm", default, skip_serializing_if = "Option::is_none")]
    pub average_bpm: Option<f64>,
    #[serde(rename = "@DateAdded", default, skip_serializing_if = "Option::is_none")]
    pub date_added: Option<String>,
    #[serde(rename = "@BitRate", default, skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<u32>,
    #[serde(rename = "@SampleRate", default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(rename = "@Comments", default, skip_serializing_if = "Option::is_none")]
    pub comments: Option<String>,
    #[serde(rename = "@PlayCount", default, skip_serializing_if = "Option::is_none")]
    pub play_count: Option<u32>,
    #[serde(rename = "@Rating", default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<u32>,
    /// `"file://localhost/path"` URI; the host is usually `localhost` or empty.
    #[serde(rename = "@Location", default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(rename = "@Remixer", default, skip_serializing_if = "Option::is_none")]
    pub remixer: Option<String>,
    /// Camelot key (`"8A"` etc.) — rekordbox stores it under this name.
    #[serde(rename = "@Tonality", default, skip_serializing_if = "Option::is_none")]
    pub tonality: Option<String>,
    #[serde(rename = "@Label", default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(rename = "@Mix", default, skip_serializing_if = "Option::is_none")]
    pub mix: Option<String>,

    #[serde(rename = "TEMPO", default, skip_serializing_if = "Vec::is_empty")]
    pub tempos: Vec<Tempo>,
    #[serde(rename = "POSITION_MARK", default, skip_serializing_if = "Vec::is_empty")]
    pub position_marks: Vec<PositionMark>,
}

/// One `<TEMPO>` beatgrid anchor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename = "TEMPO")]
pub struct Tempo {
    /// Start of the anchor in seconds.
    #[serde(rename = "@Inizio", default)]
    pub inizio: f64,
    /// BPM in effect from this anchor onward.
    #[serde(rename = "@Bpm", default)]
    pub bpm: f64,
    /// e.g. `"4/4"`.
    #[serde(rename = "@Metro", default = "default_metro")]
    pub metro: String,
    /// Beat in bar (1..=N).
    #[serde(rename = "@Battito", default = "default_battito")]
    pub battito: u32,
}

/// One `<POSITION_MARK>`.
///
/// `Num` = -1 → memory cue. 0..=7 → hot cue slot.
/// `Type` rekordbox encodes the kind: 0 cue, 1 fade-in, 2 fade-out,
/// 3 load, 4 loop. We accept anything and let the mapping layer interpret.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename = "POSITION_MARK")]
pub struct PositionMark {
    #[serde(rename = "@Name", default)]
    pub name: String,
    #[serde(rename = "@Type", default)]
    pub kind: u8,
    #[serde(rename = "@Start", default)]
    pub start: f64,
    #[serde(rename = "@End", default, skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
    #[serde(rename = "@Num", default = "default_num")]
    pub num: i32,
}

fn default_metro() -> String {
    "4/4".to_string()
}

fn default_battito() -> u32 {
    1
}

fn default_num() -> i32 {
    -1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_collection_with_one_track() {
        let xml = r#"<COLLECTION Entries="1">
  <TRACK TrackID="1" Name="Track A" Artist="Foo" AverageBpm="128.00" Tonality="8A"
         Location="file://localhost/Users/me/Music/a.mp3" TotalTime="240">
    <TEMPO Inizio="0.024" Bpm="128.00" Metro="4/4" Battito="1"/>
    <POSITION_MARK Name="Drop" Type="0" Start="32.5" Num="0"/>
  </TRACK>
</COLLECTION>"#;
        let c: Collection = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(c.entries, 1);
        assert_eq!(c.tracks.len(), 1);
        let t = &c.tracks[0];
        assert_eq!(t.track_id, Some(1));
        assert_eq!(t.name.as_deref(), Some("Track A"));
        assert_eq!(t.average_bpm, Some(128.0));
        assert_eq!(t.tonality.as_deref(), Some("8A"));
        assert_eq!(t.tempos.len(), 1);
        assert!((t.tempos[0].inizio - 0.024).abs() < 1e-9);
        assert_eq!(t.position_marks.len(), 1);
        assert_eq!(t.position_marks[0].num, 0);
        assert_eq!(t.position_marks[0].name, "Drop");
    }

    #[test]
    fn defaults_for_missing_attributes() {
        let xml = r#"<TRACK TrackID="2"><TEMPO Inizio="0.0" Bpm="120.0"/></TRACK>"#;
        let t: Track = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(t.track_id, Some(2));
        assert!(t.name.is_none());
        assert_eq!(t.tempos.len(), 1);
        assert_eq!(t.tempos[0].metro, "4/4");
        assert_eq!(t.tempos[0].battito, 1);
    }

    #[test]
    fn track_round_trips_through_serde() {
        let original = Track {
            track_id: Some(42),
            name: Some("Round Trip".into()),
            artist: Some("Tester".into()),
            average_bpm: Some(124.5),
            tonality: Some("4A".into()),
            location: Some("file://localhost/tmp/a.mp3".into()),
            tempos: vec![Tempo {
                inizio: 0.0,
                bpm: 124.5,
                metro: "4/4".into(),
                battito: 1,
            }],
            position_marks: vec![PositionMark {
                name: "Drop".into(),
                kind: 0,
                start: 32.0,
                end: None,
                num: 0,
            }],
            ..Default::default()
        };
        let serialized = quick_xml::se::to_string(&original).unwrap();
        let back: Track = quick_xml::de::from_str(&serialized).unwrap();
        assert_eq!(back, original);
    }
}
