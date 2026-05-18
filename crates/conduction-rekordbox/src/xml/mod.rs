//! rekordbox XML format (`DJ_PLAYLISTS` root, exported by rekordbox.app).
//!
//! The model is intentionally permissive: every field is optional, every
//! collection defaults to empty. rekordbox.app has shipped multiple subtly
//! different attribute sets across versions, and we want to round-trip
//! unfamiliar files without losing data.
//!
//! Phase 1 ships the data model only. Conduction <-> rekordbox mapping
//! and a registered `Exporter`/`Importer` plugin land in Phase 2.

pub mod collection;
pub mod header;

pub use collection::{Collection, PositionMark, Tempo, Track};
pub use header::{DjPlaylists, Product};
