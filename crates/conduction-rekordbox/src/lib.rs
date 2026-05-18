//! conduction-rekordbox — rekordbox interop.
//!
//! Two read/write surfaces:
//!
//! - [`xml`] handles the `DJ_PLAYLISTS` XML format exported by
//!   rekordbox.app (Library → Export Collection in xml format).
//! - USB / PDB / ANLZ support will live alongside [`xml`] in later phases.
//!
//! No `conduction-export::Exporter` / `Importer` impl is wired yet — those
//! land in Phase 2 once the XML model is solid.

#![forbid(unsafe_code)]

pub mod exporter;
pub mod mapping;
pub mod xml;

pub use exporter::RekordboxXmlExporter;
