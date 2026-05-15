//! Format → plugin lookup used by the Tauri command layer.
//!
//! Plugins (cset / rekordbox-xml / rekordbox-usb / serato) register
//! themselves into a [`PluginRegistry`] at app boot; the host then resolves
//! the user's selected [`Format`] to the right [`Exporter`] / [`Importer`]
//! via this registry.

use std::collections::HashMap;
use std::sync::Arc;

use crate::{Exporter, Format, FormatInfo, Importer};

#[derive(Default)]
pub struct PluginRegistry {
    exporters: HashMap<Format, Arc<dyn Exporter>>,
    importers: HashMap<Format, Arc<dyn Importer>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_exporter<E: Exporter + 'static>(&mut self, exporter: E) {
        self.exporters.insert(exporter.format(), Arc::new(exporter));
    }

    pub fn register_importer<I: Importer + 'static>(&mut self, importer: I) {
        self.importers.insert(importer.format(), Arc::new(importer));
    }

    pub fn exporter(&self, format: Format) -> Option<Arc<dyn Exporter>> {
        self.exporters.get(&format).cloned()
    }

    pub fn importer(&self, format: Format) -> Option<Arc<dyn Importer>> {
        self.importers.get(&format).cloned()
    }

    /// Returns every known format with an `available` flag so the UI can
    /// render unavailable variants as disabled / "coming soon" rows.
    pub fn export_formats(&self) -> Vec<FormatInfo> {
        Format::all()
            .iter()
            .map(|f| FormatInfo::from_format(*f, self.exporters.contains_key(f)))
            .collect()
    }

    pub fn import_formats(&self) -> Vec<FormatInfo> {
        Format::all()
            .iter()
            .map(|f| FormatInfo::from_format(*f, self.importers.contains_key(f)))
            .collect()
    }
}

/// Default registry. Empty for now — plugin registration lands per-format in
/// subsequent phases (Phase 1+ for rekordbox XML, Phase 6+ for Serato).
pub fn default_registry() -> PluginRegistry {
    PluginRegistry::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        ExportOptions, Exporter, ImportOptions, Importer, LibraryExportReport,
        LibraryImportReport,
    };
    use crate::ExportError;
    use conduction_library::Library;

    struct StubExporter(Format);
    impl Exporter for StubExporter {
        fn format(&self) -> Format {
            self.0
        }
        fn export(
            &self,
            _library: &Library,
            _options: &ExportOptions,
        ) -> Result<LibraryExportReport, ExportError> {
            Err(ExportError::NotImplemented)
        }
    }

    struct StubImporter(Format);
    impl Importer for StubImporter {
        fn format(&self) -> Format {
            self.0
        }
        fn import(
            &self,
            _library: &Library,
            _options: &ImportOptions,
        ) -> Result<LibraryImportReport, ExportError> {
            Err(ExportError::NotImplemented)
        }
    }

    #[test]
    fn empty_registry_lists_all_formats_as_unavailable() {
        let r = PluginRegistry::new();
        let exp = r.export_formats();
        assert_eq!(exp.len(), Format::all().len());
        assert!(exp.iter().all(|f| !f.available));
    }

    #[test]
    fn registering_an_exporter_marks_only_that_format_available() {
        let mut r = PluginRegistry::new();
        r.register_exporter(StubExporter(Format::RekordboxXml));
        let exp = r.export_formats();
        let xml = exp.iter().find(|f| f.format == Format::RekordboxXml).unwrap();
        let serato = exp.iter().find(|f| f.format == Format::Serato).unwrap();
        assert!(xml.available);
        assert!(!serato.available);
    }

    #[test]
    fn exporter_and_importer_are_independent() {
        let mut r = PluginRegistry::new();
        r.register_exporter(StubExporter(Format::Cset));
        r.register_importer(StubImporter(Format::Serato));

        assert!(r.exporter(Format::Cset).is_some());
        assert!(r.importer(Format::Cset).is_none());
        assert!(r.exporter(Format::Serato).is_none());
        assert!(r.importer(Format::Serato).is_some());
    }
}
