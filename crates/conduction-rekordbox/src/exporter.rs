//! `Exporter` impl that serializes a Library snapshot as rekordbox XML.

use std::fs;

use conduction_export::{ExportError, ExportOptions, Exporter, Format, LibraryExportReport};
use conduction_library::Library;

use crate::mapping::library_to_dj_playlists;

/// Stateless. Construct once at app boot and register with the
/// `PluginRegistry`.
#[derive(Debug, Default, Clone, Copy)]
pub struct RekordboxXmlExporter;

impl RekordboxXmlExporter {
    pub fn new() -> Self {
        Self
    }
}

impl Exporter for RekordboxXmlExporter {
    fn format(&self) -> Format {
        Format::RekordboxXml
    }

    fn export(
        &self,
        library: &mut Library,
        options: &ExportOptions,
    ) -> Result<LibraryExportReport, ExportError> {
        let dj =
            library_to_dj_playlists(library).map_err(|e| ExportError::Library(e.to_string()))?;

        let body = quick_xml::se::to_string(&dj)
            .map_err(|e| ExportError::Library(format!("xml serialize: {e}")))?;
        let payload = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{body}\n");

        let mut bytes_written = 0u64;
        if !options.dry_run {
            fs::write(&options.destination, &payload)?;
            bytes_written = payload.len() as u64;
        }

        let tracks_written = dj.collection.as_ref().map(|c| c.tracks.len()).unwrap_or(0);

        Ok(LibraryExportReport {
            format: Format::RekordboxXml,
            tracks_written,
            bytes_written,
            warnings: Vec::new(),
        })
    }
}
