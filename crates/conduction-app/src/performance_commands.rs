//! Performance endpoints use the same dispatcher as native controllers.
use crate::performance::{LinkInterface, PerformanceBrowser, PerformanceHandle, PerformanceStatus};
use conduction_audio::{AudioOutputConfig, AudioOutputDescriptor};
use conduction_link::LinkConfig;
use conduction_midi::{ControllerAction, MidiConfig, MidiDevices, MidiProfile, MidiService};
use tauri::State;

type Result<T = ()> = std::result::Result<T, String>;

#[tauri::command]
pub fn get_performance_status(performance: State<'_, PerformanceHandle>) -> PerformanceStatus {
    performance.status()
}
#[tauri::command]
pub async fn perform(
    performance: State<'_, PerformanceHandle>,
    action: ControllerAction,
) -> Result {
    performance.perform(action).await.map_err(|e| e.to_string())
}
#[tauri::command]
pub fn list_audio_outputs() -> Vec<AudioOutputDescriptor> {
    conduction_audio::list_audio_outputs()
}
#[tauri::command]
pub async fn configure_audio(
    performance: State<'_, PerformanceHandle>,
    config: AudioOutputConfig,
) -> Result {
    performance
        .configure_audio(config)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn list_link_interfaces() -> Result<Vec<LinkInterface>> {
    crate::performance::list_link_interfaces().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn configure_link(
    performance: State<'_, PerformanceHandle>,
    config: LinkConfig,
) -> Result {
    performance
        .configure_link(config)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn refresh_link_library(performance: State<'_, PerformanceHandle>) -> Result {
    performance
        .refresh_library()
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn request_link_master(performance: State<'_, PerformanceHandle>) -> Result {
    performance.request_master().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn list_midi_devices(performance: State<'_, PerformanceHandle>) -> Result<MidiDevices> {
    performance.midi().devices().map_err(|e| e.to_string())
}
#[tauri::command]
pub fn list_midi_profiles() -> Vec<MidiProfile> {
    MidiService::profiles()
}
#[tauri::command]
pub fn configure_midi(performance: State<'_, PerformanceHandle>, config: MidiConfig) -> Result {
    performance
        .configure_midi(config)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn disconnect_midi(
    performance: State<'_, PerformanceHandle>,
    input_port: Option<String>,
) -> Result {
    performance
        .disconnect_midi(input_port)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn set_performance_browser(
    performance: State<'_, PerformanceHandle>,
    browser: PerformanceBrowser,
) -> Result {
    performance.set_browser(browser).map_err(|e| e.to_string())
}
