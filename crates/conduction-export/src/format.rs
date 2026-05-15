//! Library-wide export/import formats.
//!
//! Each variant is one "shape" that the user can pick from the Library screen.
//! Plugins implementing [`crate::api::Exporter`] / [`crate::api::Importer`]
//! advertise the variant(s) they support.

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    /// Conduction's own library snapshot (single JSON-ish file).
    Cset,
    /// rekordbox.app `DJ_PLAYLISTS` XML — single file, references audio by path.
    RekordboxXml,
    /// CDJ-2000 compatible USB layout (`PIONEER/` + `Contents/`).
    RekordboxUsb,
    /// Serato — writes Hot Cue / Beatgrid into ID3 `GEOB` frames in-place.
    Serato,
}

/// Where a format expects to read from / write to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TargetKind {
    /// A single file (e.g. `library.cset`, `rekordbox.xml`).
    File,
    /// A directory the plugin populates (e.g. a USB volume root).
    Directory,
    /// In-place mutation of audio files referenced by the library.
    InPlace,
}

impl Format {
    /// Stable identifier used over the IPC boundary and in CLI flags.
    pub fn id(&self) -> &'static str {
        match self {
            Self::Cset => "cset",
            Self::RekordboxXml => "rekordbox-xml",
            Self::RekordboxUsb => "rekordbox-usb",
            Self::Serato => "serato",
        }
    }

    /// Human-facing display name.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cset => "Conduction (.cset)",
            Self::RekordboxXml => "rekordbox XML",
            Self::RekordboxUsb => "rekordbox USB",
            Self::Serato => "Serato",
        }
    }

    pub fn target_kind(&self) -> TargetKind {
        match self {
            Self::Cset | Self::RekordboxXml => TargetKind::File,
            Self::RekordboxUsb => TargetKind::Directory,
            Self::Serato => TargetKind::InPlace,
        }
    }

    pub fn all() -> &'static [Format] {
        &[
            Self::Cset,
            Self::RekordboxXml,
            Self::RekordboxUsb,
            Self::Serato,
        ]
    }
}

/// Lightweight descriptor served over IPC so the UI can render a format
/// selector without depending on the plugin registry directly.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct FormatInfo {
    pub format: Format,
    pub id: String,
    pub label: String,
    pub target_kind: TargetKind,
    /// `true` when a plugin is registered for this format on the current build.
    pub available: bool,
}

impl FormatInfo {
    pub fn from_format(format: Format, available: bool) -> Self {
        Self {
            format,
            id: format.id().to_string(),
            label: format.label().to_string(),
            target_kind: format.target_kind(),
            available,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_kebab_case_and_unique() {
        let ids: Vec<&'static str> = Format::all().iter().map(|f| f.id()).collect();
        assert_eq!(ids, ["cset", "rekordbox-xml", "rekordbox-usb", "serato"]);
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), Format::all().len(), "format ids must be unique");
    }

    #[test]
    fn serde_matches_id() {
        let json = serde_json::to_string(&Format::RekordboxXml).unwrap();
        assert_eq!(json, "\"rekordbox-xml\"");
        let back: Format = serde_json::from_str("\"serato\"").unwrap();
        assert_eq!(back, Format::Serato);
    }
}
