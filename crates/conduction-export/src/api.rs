//! Library-level export/import trait surface.
//!
//! Each plugin (cset / rekordbox-xml / rekordbox-usb / serato) implements
//! either [`Exporter`], [`Importer`], or both. The host (conduction-app)
//! resolves the right plugin via [`crate::registry::PluginRegistry`] and
//! dispatches on the user's [`Format`] choice.

use std::path::PathBuf;

use conduction_library::Library;
use serde::{Deserialize, Serialize};

use crate::{ExportError, Format};

/// Inputs the host gives an exporter when the user hits "Export".
///
/// Kept off the IPC schema deliberately — the `conduction-app` layer maps
/// plain-old strings/JSON from Tauri into this internal struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    /// File path or directory the plugin writes to; semantics follow
    /// `Format::target_kind`.
    pub destination: PathBuf,
    /// When set, the plugin must compute everything but skip filesystem writes.
    #[serde(default)]
    pub dry_run: bool,
    /// Format-specific overrides (e.g. encryption key, USB volume label).
    /// JSON so the IPC layer doesn't need a per-format DTO.
    #[serde(default)]
    pub extra: Option<serde_json::Value>,
}

/// How an importer should treat a track that already exists in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictStrategy {
    /// Leave the existing row untouched.
    Skip,
    /// Merge incoming metadata into the existing row.
    Update,
    /// Replace the existing row in full.
    Replace,
}

impl Default for ConflictStrategy {
    fn default() -> Self {
        Self::Skip
    }
}

/// Inputs the host gives an importer when the user hits "Import".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOptions {
    pub source: PathBuf,
    #[serde(default)]
    pub conflict_strategy: ConflictStrategy,
    #[serde(default)]
    pub extra: Option<serde_json::Value>,
}

/// Result returned to the UI after a successful export.
///
/// Distinct from the legacy `ExportReport` in `lib.rs`, which is the
/// rekordbox-USB-specific summary used by the older `execute(plan)` API.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LibraryExportReport {
    pub format: Format,
    pub tracks_written: usize,
    pub bytes_written: u64,
    /// Non-fatal issues the user should see (e.g. "track has no beatgrid,
    /// skipped TEMPO export").
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Result returned to the UI after a successful import.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct LibraryImportReport {
    pub format: Format,
    pub tracks_imported: usize,
    pub tracks_updated: usize,
    pub tracks_skipped: usize,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// One direction (out) of a format plugin.
///
/// Takes `&mut Library` because `conduction-library::Library` uses a raw
/// `rusqlite::Connection` (no internal Mutex), and even read-only queries on
/// SQLite go through `&mut self` to keep the borrow story honest about
/// statement caching.
pub trait Exporter: Send + Sync {
    fn format(&self) -> Format;
    fn label(&self) -> &'static str {
        self.format().label()
    }

    fn export(
        &self,
        library: &mut Library,
        options: &ExportOptions,
    ) -> Result<LibraryExportReport, ExportError>;
}

/// One direction (in) of a format plugin.
pub trait Importer: Send + Sync {
    fn format(&self) -> Format;
    fn label(&self) -> &'static str {
        self.format().label()
    }

    fn import(
        &self,
        library: &mut Library,
        options: &ImportOptions,
    ) -> Result<LibraryImportReport, ExportError>;
}
