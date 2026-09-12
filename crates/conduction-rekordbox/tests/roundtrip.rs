//! Integration tests: parse fixture XML, serialize back, parse again,
//! assert structural equality.
//!
//! These tests guard the read/write contract on a per-attribute basis.
//! Whenever a new attribute is added to the model, drop a real-world
//! fixture into `tests/fixtures/` and add a case here.

use std::path::PathBuf;

use conduction_rekordbox::xml::DjPlaylists;
use pretty_assertions::assert_eq;

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read fixture {}: {}", path.display(), e))
}

fn roundtrip(xml: &str) -> DjPlaylists {
    let first: DjPlaylists = quick_xml::de::from_str(xml).expect("first deserialize must succeed");
    let serialized = quick_xml::se::to_string(&first).expect("serialize must succeed");
    let second: DjPlaylists = quick_xml::de::from_str(&serialized).unwrap_or_else(|e| {
        panic!(
            "second deserialize failed: {}\n--- serialized output:\n{}",
            e, serialized
        )
    });
    assert_eq!(first, second, "round-trip changed the model");
    second
}

#[test]
fn minimal_fixture_roundtrips() {
    let dj = roundtrip(&fixture("minimal.xml"));
    assert_eq!(dj.version, "1.0.0");
    let product = dj.product.expect("PRODUCT");
    assert_eq!(product.name, "rekordbox");
    let collection = dj.collection.expect("COLLECTION");
    assert_eq!(collection.entries, 0);
    assert_eq!(collection.tracks.len(), 0);
    let playlists = dj.playlists.expect("PLAYLISTS");
    let root = playlists.root.expect("ROOT NODE");
    assert_eq!(root.name, "ROOT");
    assert_eq!(root.kind, 0);
    assert_eq!(root.children.len(), 0);
}

#[test]
fn sample_fixture_roundtrips() {
    let dj = roundtrip(&fixture("sample.xml"));
    let collection = dj.collection.expect("COLLECTION");
    assert_eq!(collection.entries, 2);
    assert_eq!(collection.tracks.len(), 2);

    // Track 1 — three POSITION_MARKs (two hot cues + one memory cue)
    let t1 = &collection.tracks[0];
    assert_eq!(t1.track_id, Some(1));
    assert_eq!(t1.name.as_deref(), Some("Sunrise"));
    assert_eq!(t1.average_bpm, Some(124.0));
    assert_eq!(t1.tonality.as_deref(), Some("8A"));
    assert_eq!(t1.position_marks.len(), 3);
    let hot_cues: Vec<i32> = t1
        .position_marks
        .iter()
        .map(|m| m.num)
        .filter(|n| *n >= 0)
        .collect();
    assert_eq!(hot_cues, vec![0, 1]);
    let memory_cues: usize = t1.position_marks.iter().filter(|m| m.num == -1).count();
    assert_eq!(memory_cues, 1);

    // Track 2 — two TEMPO anchors (variable BPM)
    let t2 = &collection.tracks[1];
    assert_eq!(t2.tempos.len(), 2);
    assert_eq!(t2.tempos[0].bpm, 126.0);
    assert_eq!(t2.tempos[1].bpm, 128.0);

    // PLAYLISTS — one playlist referencing both tracks
    let playlists = dj.playlists.expect("PLAYLISTS");
    let root = playlists.root.expect("ROOT");
    assert_eq!(root.children.len(), 1);
    let set = &root.children[0];
    assert_eq!(set.name, "Aurora Set");
    assert_eq!(set.kind, 1);
    assert_eq!(set.tracks.len(), 2);
    assert_eq!(set.tracks[0].key, "1");
    assert_eq!(set.tracks[1].key, "2");
}
