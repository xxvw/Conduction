//! Integration test for `RekordboxXmlExporter`.
//!
//! Builds an in-memory Conduction Library, exports it to a tempfile,
//! then re-parses the XML and checks the structural mapping holds.

use std::path::PathBuf;
use std::time::Duration;

use conduction_core::{Beat, Cue, CueType, Key, KeyMode, Track};
use conduction_export::{ExportOptions, Exporter, Format};
use conduction_library::Library;
use conduction_rekordbox::{xml::DjPlaylists, RekordboxXmlExporter};

fn sample_key() -> Key {
    Key::new(8, KeyMode::Minor).unwrap()
}

fn sample_track() -> Track {
    let mut t = Track::placeholder(PathBuf::from("/tmp/exporter_demo.mp3"), sample_key());
    t.title = "Exporter Demo".into();
    t.artist = "Conduction".into();
    t.album = "Test".into();
    t.genre = "Tech".into();
    t.duration = Duration::from_secs_f64(240.0);
    t.bpm = 124.0;
    t.energy = 0.6;
    t
}

#[test]
fn exports_single_track_with_cues_and_beats_to_xml_file() {
    let mut lib = Library::in_memory().unwrap();
    let track = sample_track();
    lib.insert_track(&track).unwrap();

    lib.replace_beatgrid(
        track.id,
        &[
            Beat::new(0.024, true),
            Beat::new(0.508, false),
            Beat::new(0.992, false),
            Beat::new(1.476, false),
        ],
    )
    .unwrap();

    lib.set_hot_cue(track.id, 1, 0.024).unwrap();
    lib.set_hot_cue(track.id, 2, 32.5).unwrap();

    let drop = Cue::new(
        track.id,
        64.0,
        CueType::Drop,
        124.0,
        sample_key(),
        0.9,
        32,
    )
    .unwrap();
    lib.insert_cue(&drop).unwrap();

    let tmp = tempfile::NamedTempFile::with_suffix(".xml").unwrap();
    let destination = tmp.path().to_path_buf();

    let report = RekordboxXmlExporter::new()
        .export(
            &mut lib,
            &ExportOptions {
                destination: destination.clone(),
                dry_run: false,
                extra: None,
            },
        )
        .unwrap();

    assert_eq!(report.format, Format::RekordboxXml);
    assert_eq!(report.tracks_written, 1);
    assert!(report.bytes_written > 0);
    assert!(report.warnings.is_empty());

    // Re-parse and verify the mapping.
    let xml = std::fs::read_to_string(&destination).unwrap();
    let dj: DjPlaylists = quick_xml::de::from_str(&xml).expect("re-parse");
    let collection = dj.collection.expect("COLLECTION");
    assert_eq!(collection.tracks.len(), 1);
    let t = &collection.tracks[0];

    assert_eq!(t.track_id, Some(1));
    assert_eq!(t.name.as_deref(), Some("Exporter Demo"));
    assert_eq!(t.artist.as_deref(), Some("Conduction"));
    assert_eq!(t.album.as_deref(), Some("Test"));
    assert_eq!(t.genre.as_deref(), Some("Tech"));
    assert_eq!(t.total_time, Some(240));
    assert_eq!(t.average_bpm, Some(124.0));
    // Camelot 8A for Key::new(8, Minor)
    assert_eq!(t.tonality.as_deref(), Some("8A"));
    assert!(t.location.as_deref().unwrap().starts_with("file://localhost/"));

    // One TEMPO anchor at the first beat.
    assert_eq!(t.tempos.len(), 1);
    assert!((t.tempos[0].inizio - 0.024).abs() < 1e-9);
    assert_eq!(t.tempos[0].bpm, 124.0);

    // 2 hot cues (Num >= 0) + 1 memory cue from typed Drop (Num = -1).
    let hot_cue_nums: Vec<i32> =
        t.position_marks.iter().map(|m| m.num).filter(|n| *n >= 0).collect();
    assert_eq!(hot_cue_nums, vec![0, 1]);
    let memory_cue_count =
        t.position_marks.iter().filter(|m| m.num == -1).count();
    assert_eq!(memory_cue_count, 1);
}

#[test]
fn dry_run_does_not_write_a_file() {
    let mut lib = Library::in_memory().unwrap();
    lib.insert_track(&sample_track()).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("should-not-exist.xml");

    let report = RekordboxXmlExporter::new()
        .export(
            &mut lib,
            &ExportOptions {
                destination: destination.clone(),
                dry_run: true,
                extra: None,
            },
        )
        .unwrap();

    assert_eq!(report.tracks_written, 1);
    assert_eq!(report.bytes_written, 0);
    assert!(!destination.exists(), "dry_run must not touch the filesystem");
}
