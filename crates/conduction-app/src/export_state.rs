//! Tauri State wrapper around the conduction-export plugin registry.
//!
//! Held as an `Arc<PluginRegistry>` because registration only happens at
//! boot (in `run()`), after which the registry is read-only for the
//! lifetime of the process. Cloning the handle is cheap.

use std::sync::Arc;

use conduction_export::PluginRegistry;

#[derive(Clone)]
pub struct ExportRegistryHandle(pub Arc<PluginRegistry>);

impl ExportRegistryHandle {
    pub fn new(registry: PluginRegistry) -> Self {
        Self(Arc::new(registry))
    }
}
