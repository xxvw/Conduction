//! `<PLAYLISTS>` and its recursive `<NODE>` tree.
//!
//! ```xml
//! <PLAYLISTS>
//!   <NODE Type="0" Name="ROOT" Count="2">
//!     <NODE Type="0" Name="House" Count="1">
//!       <NODE Type="1" Name="Set 2026-05" KeyType="0" Entries="3">
//!         <TRACK Key="1"/>
//!         <TRACK Key="2"/>
//!         <TRACK Key="3"/>
//!       </NODE>
//!     </NODE>
//!   </NODE>
//! </PLAYLISTS>
//! ```
//!
//! `Type="0"` is a folder; `Type="1"` is a playlist. Playlist NODEs hold
//! `<TRACK Key="...">` references (distinct from `<TRACK>` inside
//! `<COLLECTION>`, even though they share the tag name). `KeyType="0"`
//! means the Key is a TrackID; `"1"` means it's a Location URI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename = "PLAYLISTS")]
pub struct Playlists {
    /// The root NODE is always a folder named `"ROOT"`.
    #[serde(rename = "NODE", default, skip_serializing_if = "Option::is_none")]
    pub root: Option<Node>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename = "NODE")]
pub struct Node {
    #[serde(rename = "@Name", default)]
    pub name: String,
    /// 0 = folder, 1 = playlist.
    #[serde(rename = "@Type", default)]
    pub kind: u8,
    /// Folder only — count of immediate child NODEs.
    #[serde(rename = "@Count", default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Playlist only — count of TRACK references.
    #[serde(rename = "@Entries", default, skip_serializing_if = "Option::is_none")]
    pub entries: Option<u32>,
    /// Playlist only — 0 = TrackID, 1 = Location.
    #[serde(rename = "@KeyType", default, skip_serializing_if = "Option::is_none")]
    pub key_type: Option<u8>,
    /// Sub-folders / sub-playlists.
    #[serde(rename = "NODE", default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
    /// TRACK references inside a playlist NODE.
    #[serde(rename = "TRACK", default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<PlaylistTrackRef>,
}

/// Track reference inside a playlist NODE (distinct from the rich `Track`
/// row in `COLLECTION`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename = "TRACK")]
pub struct PlaylistTrackRef {
    /// TrackID (when KeyType=0) or Location URI (when KeyType=1).
    #[serde(rename = "@Key", default)]
    pub key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_folder_with_one_playlist_child() {
        let xml = r#"<PLAYLISTS>
  <NODE Type="0" Name="ROOT" Count="1">
    <NODE Type="1" Name="Set A" KeyType="0" Entries="2">
      <TRACK Key="1"/>
      <TRACK Key="2"/>
    </NODE>
  </NODE>
</PLAYLISTS>"#;
        let p: Playlists = quick_xml::de::from_str(xml).unwrap();
        let root = p.root.expect("ROOT NODE must parse");
        assert_eq!(root.kind, 0);
        assert_eq!(root.name, "ROOT");
        assert_eq!(root.children.len(), 1);
        let leaf = &root.children[0];
        assert_eq!(leaf.kind, 1);
        assert_eq!(leaf.key_type, Some(0));
        assert_eq!(leaf.tracks.len(), 2);
        assert_eq!(leaf.tracks[0].key, "1");
        assert_eq!(leaf.tracks[1].key, "2");
    }

    #[test]
    fn nested_folders_round_trip_via_serde() {
        let original = Playlists {
            root: Some(Node {
                name: "ROOT".into(),
                kind: 0,
                count: Some(1),
                children: vec![Node {
                    name: "Folder".into(),
                    kind: 0,
                    count: Some(1),
                    children: vec![Node {
                        name: "List".into(),
                        kind: 1,
                        entries: Some(1),
                        key_type: Some(0),
                        tracks: vec![PlaylistTrackRef { key: "42".into() }],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
        };
        let serialized = quick_xml::se::to_string(&original).unwrap();
        let back: Playlists = quick_xml::de::from_str(&serialized).unwrap();
        assert_eq!(back, original);
    }
}
