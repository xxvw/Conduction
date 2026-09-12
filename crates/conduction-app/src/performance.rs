//! Shared performance controls for Tauri, MIDI and the localhost API.
//! Network serving and file preparation never run on the audio callback.

use crate::{
    audio_engine::{AudioCommand, AudioHandle, TrackMetadata},
    library_state::LibraryHandle,
    link_library::{prepare_catalog_report, CatalogIssue},
    performance_bridge::{spawn_bridge, BridgeGuard},
    settings::SettingsHandle,
};
use conduction_audio::{AudioOutputConfig, AudioOutputStatus, DeckId};
use conduction_link::{LinkConfig, LinkHandle, LinkStatus};
use conduction_midi::{ControllerAction, MidiConfig, MidiService, MidiStatus};
use crossbeam::channel;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

#[path = "performance_actions.rs"]
mod actions;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceBrowser {
    pub query: String,
    pub selected_track_id: Option<String>,
    pub playlist_id: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct PerformanceStatus {
    pub audio: AudioOutputStatus,
    pub audio_config: AudioOutputConfig,
    pub link: LinkStatus,
    pub link_config: LinkConfig,
    pub midi: MidiStatus,
    pub browser: PerformanceBrowser,
    pub last_error: Option<String>,
    pub library_preparing: bool,
    pub catalog_issues: Vec<CatalogIssue>,
}

#[derive(Clone)]
pub struct PerformanceHandle {
    inner: Arc<PerformanceInner>,
}

struct PerformanceInner {
    audio: AudioHandle,
    library: LibraryHandle,
    settings: SettingsHandle,
    midi: Arc<MidiService>,
    link: Arc<Mutex<Option<LinkHandle>>>,
    browser: Mutex<PerformanceBrowser>,
    last_error: Arc<Mutex<Option<String>>>,
    preparing: AtomicBool,
    audio_configuration: tokio::sync::Mutex<()>,
    catalog_issues: Mutex<Vec<CatalogIssue>>,
    fx: Mutex<[FxSelection; 2]>,
    _bridge: BridgeGuard,
}

#[derive(Clone)]
struct FxSelection {
    reverb: bool,
    enabled: bool,
    mix: f32,
}
impl Default for FxSelection {
    fn default() -> Self {
        Self {
            reverb: false,
            enabled: false,
            mix: 0.3,
        }
    }
}

impl PerformanceHandle {
    pub fn new(
        audio: AudioHandle,
        library: LibraryHandle,
        settings: SettingsHandle,
    ) -> anyhow::Result<Self> {
        let (tx, rx) = channel::bounded::<ControllerAction>(1024);
        let overflow = Arc::new(AtomicBool::new(false));
        let overflow_callback = overflow.clone();
        let midi = Arc::new(MidiService::new(move |action| {
            if tx.try_send(action).is_err() {
                overflow_callback.store(true, Ordering::Release);
            }
        }));
        let link = Arc::new(Mutex::new(None));
        let last_error = Arc::new(Mutex::new(None));
        let bridge = spawn_bridge(
            audio.clone(),
            library.clone(),
            link.clone(),
            midi.clone(),
            last_error.clone(),
        );
        let inner = Arc::new(PerformanceInner {
            audio,
            library,
            settings,
            midi,
            link,
            browser: Mutex::new(PerformanceBrowser::default()),
            last_error,
            preparing: AtomicBool::new(false),
            catalog_issues: Mutex::new(Vec::new()),
            audio_configuration: tokio::sync::Mutex::new(()),
            fx: Mutex::new([FxSelection::default(), FxSelection::default()]),
            _bridge: bridge,
        });
        let weak = Arc::downgrade(&inner);
        thread::Builder::new()
            .name("controller-dispatch".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(error) => {
                        if let Some(inner) = weak.upgrade() {
                            *inner.last_error.lock() = Some(error.to_string());
                        }
                        return;
                    }
                };
                loop {
                    if overflow.swap(false, Ordering::AcqRel) {
                        while rx.try_recv().is_ok() {}
                        if let Some(inner) = weak.upgrade() {
                            let _ = inner.audio.send(AudioCommand::ReleaseControls);
                            *inner.last_error.lock() = Some(
                                "Controller input overflow; held controls were released".into(),
                            );
                        }
                    }
                    let action = match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(action) => action,
                        Err(channel::RecvTimeoutError::Timeout) => {
                            if weak.strong_count() == 0 {
                                break;
                            }
                            continue;
                        }
                        Err(_) => break,
                    };
                    let Some(inner) = weak.upgrade() else {
                        break;
                    };
                    let handle = Self { inner };
                    // Decoding a new track must not hold up Cue release or the other deck.
                    if let ControllerAction::LoadSelected { deck } = &action {
                        match handle.selected_load_command(deck) {
                            Ok(command) => {
                                // Capture the selection and assign the load generation in
                                // MIDI order. Only the decode acknowledgement runs later.
                                let applied = handle.inner.audio.execute(command);
                                runtime.spawn(async move {
                                    if let Err(error) = applied.await {
                                        *handle.inner.last_error.lock() = Some(error.to_string());
                                    }
                                });
                            }
                            Err(error) => *handle.inner.last_error.lock() = Some(error.to_string()),
                        }
                        continue;
                    }
                    if let Err(error) = runtime.block_on(handle.perform(action)) {
                        *handle.inner.last_error.lock() = Some(error.to_string());
                    }
                }
            })?;
        Ok(Self { inner })
    }

    pub fn status(&self) -> PerformanceStatus {
        let settings = self.inner.settings.get();
        PerformanceStatus {
            audio: self.inner.audio.audio_status(),
            audio_config: self.inner.audio.snapshot().audio_config,
            link: self
                .inner
                .link
                .lock()
                .as_ref()
                .map(LinkHandle::snapshot)
                .unwrap_or_default(),
            link_config: settings.link,
            midi: self.inner.midi.status(),
            browser: self.inner.browser.lock().clone(),
            last_error: self.inner.last_error.lock().clone(),
            library_preparing: self.inner.preparing.load(Ordering::Acquire),
            catalog_issues: self.inner.catalog_issues.lock().clone(),
        }
    }

    pub fn midi(&self) -> &MidiService {
        &self.inner.midi
    }

    pub async fn configure_audio(&self, config: AudioOutputConfig) -> anyhow::Result<()> {
        let _configuration = self.inner.audio_configuration.lock().await;
        self.inner.audio.configure_audio(config.clone()).await?;
        self.inner.settings.update(|s| {
            s.audio_main_output = config.device_name.clone();
            s.audio_cue_output = config.cue_device_name.clone();
            s.audio = Some(config);
        })?;
        *self.inner.last_error.lock() = None;
        Ok(())
    }

    /// Legacy forms share the same stopped-state validation and serialized output transaction.
    pub async fn save_legacy_settings(
        &self,
        incoming: crate::settings::AppSettings,
        intent: Option<crate::settings::LegacySettingsIntent>,
    ) -> anyhow::Result<()> {
        use crate::settings::LegacySettingsIntent;
        let _configuration = self.inner.audio_configuration.lock().await;
        let current = self.inner.settings.get();
        let active = self.inner.audio.snapshot().audio_config;
        let audio = current.legacy_audio_update(&incoming, intent, &active);
        if let Some(config) = &audio {
            self.inner.audio.configure_audio(config.clone()).await?;
        }
        self.inner.settings.update(|settings| {
            if intent.is_none() || matches!(intent, Some(LegacySettingsIntent::Keybindings)) {
                settings.keybindings = incoming.keybindings;
            }
            if let Some(config) = audio {
                settings.audio_main_output = config.device_name.clone();
                settings.audio_cue_output = config.cue_device_name.clone();
                settings.audio = Some(config);
            }
        })?;
        *self.inner.last_error.lock() = None;
        Ok(())
    }

    pub fn configure_midi(&self, config: MidiConfig) -> anyhow::Result<()> {
        self.inner.midi.configure(config.clone())?;
        self.inner.settings.update(|s| {
            s.midi.retain(|c| c.input_port != config.input_port);
            s.midi.push(config);
        })?;
        Ok(())
    }

    pub fn disconnect_midi(&self, input_port: Option<String>) -> anyhow::Result<()> {
        self.inner.midi.disconnect(input_port.clone())?;
        self.inner.settings.update(|s| match &input_port {
            Some(port) => s.midi.retain(|c| &c.input_port != port),
            None => s.midi.clear(),
        })?;
        Ok(())
    }

    pub async fn configure_link(&self, config: LinkConfig) -> anyhow::Result<()> {
        if self.inner.preparing.swap(true, Ordering::AcqRel) {
            anyhow::bail!("Library preparation is already in progress");
        }
        let handle = self.clone();
        let result =
            tauri::async_runtime::spawn_blocking(move || handle.configure_link_blocking(config))
                .await;
        self.inner.preparing.store(false, Ordering::Release);
        let result = result.map_err(anyhow::Error::from).and_then(|r| r);
        if let Err(error) = &result {
            *self.inner.last_error.lock() = Some(error.to_string());
        }
        result
    }

    fn configure_link_blocking(&self, config: LinkConfig) -> anyhow::Result<()> {
        if config.enabled {
            let interfaces = list_link_interfaces()?;
            anyhow::ensure!(
                interfaces
                    .iter()
                    .any(|i| i.ip == config.interface_ip.to_string()
                        && i.broadcast == config.broadcast_ip.to_string()
                        && i.mac_address == config.mac_address),
                "Choose an available network interface"
            );
        }
        let catalog = if config.enabled && config.library_enabled {
            let report = prepare_catalog_report(&self.inner.library, cache_dir()?)?;
            *self.inner.catalog_issues.lock() = report.issues;
            report.snapshot
        } else {
            Default::default()
        };
        let mut link = self.inner.link.lock();
        if let Some(old) = link.take() {
            old.stop();
        }
        if config.enabled {
            *link = Some(LinkHandle::start(config.clone(), catalog)?);
        }
        self.inner.settings.update(|s| s.link = config)?;
        *self.inner.last_error.lock() = None;
        Ok(())
    }

    pub async fn refresh_library(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.inner.link.lock().is_some(),
            "Pro DJ Link is not connected"
        );
        if self.inner.preparing.swap(true, Ordering::AcqRel) {
            anyhow::bail!("Library preparation is already in progress");
        }
        let handle = self.clone();
        let result = tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<()> {
            let report = prepare_catalog_report(&handle.inner.library, cache_dir()?)?;
            *handle.inner.catalog_issues.lock() = report.issues;
            let link = handle.inner.link.lock();
            let link = link
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Pro DJ Link disconnected"))?;
            anyhow::ensure!(link.config().library_enabled, "Library sharing is disabled");
            link.update_library(report.snapshot);
            Ok(())
        })
        .await;
        self.inner.preparing.store(false, Ordering::Release);
        result?
    }

    pub fn request_master(&self) -> anyhow::Result<()> {
        let link = self.inner.link.lock();
        let link = link
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Pro DJ Link is not connected"))?;
        anyhow::ensure!(
            link.snapshot().player_number.is_some(),
            "No free player number for master participation"
        );
        let snap = self.inner.audio.snapshot();
        let deck = if link.config().source_deck == "B" {
            &snap.deck_b
        } else {
            &snap.deck_a
        };
        anyhow::ensure!(
            deck.bpm.is_some_and(|v| v > 0.0) && deck.beat_phase.is_some(),
            "Load an analyzed track before requesting master"
        );
        link.request_master();
        Ok(())
    }

    pub fn set_browser(&self, browser: PerformanceBrowser) -> anyhow::Result<()> {
        anyhow::ensure!(browser.query.len() <= 1024, "Search is too long");
        if let Some(id) = &browser.selected_track_id {
            uuid::Uuid::parse_str(id)?;
        }
        if let Some(id) = &browser.playlist_id {
            uuid::Uuid::parse_str(id)?;
        }
        *self.inner.browser.lock() = browser;
        Ok(())
    }

    pub async fn load_track(&self, deck: DeckId, path: PathBuf) -> anyhow::Result<()> {
        load_track(&self.inner.audio, &self.inner.library, deck, path).await
    }

    /// Restore configured connections without making application startup fail.
    pub async fn restore(&self) {
        let settings = self.inner.settings.get();
        for config in settings.midi {
            if let Err(error) = self.inner.midi.configure(config) {
                *self.inner.last_error.lock() = Some(error.to_string());
            }
        }
        if settings.link.enabled {
            if let Err(error) = self.configure_link(settings.link).await {
                *self.inner.last_error.lock() = Some(error.to_string());
            }
        }
    }
}

pub async fn load_track(
    audio: &AudioHandle,
    library: &LibraryHandle,
    deck: DeckId,
    path: PathBuf,
) -> anyhow::Result<()> {
    let metadata = track_metadata(library, &path)?;
    audio.load_track(deck, path, metadata).await
}

pub fn track_metadata(library: &LibraryHandle, path: &Path) -> anyhow::Result<TrackMetadata> {
    library.with_library(|lib| metadata_from_library(lib, path))
}

pub(super) fn metadata_from_library(
    lib: &conduction_library::Library,
    path: &Path,
) -> anyhow::Result<TrackMetadata> {
    let Some(track) = lib.get_track_by_path(path)? else {
        return Ok(TrackMetadata::default());
    };
    let beats = lib
        .load_beatgrid(track.id)?
        .iter()
        .map(|b| b.position_sec)
        .collect();
    let mut hot_cues = vec![None; 8];
    for (slot, position) in lib.list_hot_cues(track.id)? {
        if (1..=8).contains(&slot) {
            hot_cues[usize::from(slot - 1)] = Some(position);
        }
    }
    Ok(TrackMetadata {
        track_id: Some(track.id.to_string()),
        bpm: (track.bpm > 0.0).then_some(track.bpm),
        beats,
        hot_cues,
    })
}

fn cache_dir() -> anyhow::Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("com", "xxvw", "Conduction")
        .ok_or_else(|| anyhow::anyhow!("No app data directory"))?;
    Ok(dirs.cache_dir().join("link"))
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkInterface {
    pub name: String,
    pub ip: String,
    pub broadcast: String,
    pub mac_address: [u8; 6],
}

pub fn list_link_interfaces() -> anyhow::Result<Vec<LinkInterface>> {
    use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
    let mut result = Vec::new();
    for interface in NetworkInterface::show()? {
        if interface.internal {
            continue;
        }
        let Some(mac) = interface.mac_addr.as_deref().and_then(parse_mac) else {
            continue;
        };
        for addr in interface.addr {
            if let Addr::V4(addr) = addr {
                let Some(broadcast) = addr.broadcast else {
                    continue;
                };
                result.push(LinkInterface {
                    name: interface.name.clone(),
                    ip: addr.ip.to_string(),
                    broadcast: broadcast.to_string(),
                    mac_address: mac,
                });
            }
        }
    }
    Ok(result)
}

fn parse_mac(value: &str) -> Option<[u8; 6]> {
    let bytes = value
        .split([':', '-'])
        .map(|s| u8::from_str_radix(s, 16))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    bytes.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_mac_addresses_without_partial_acceptance() {
        assert_eq!(
            parse_mac("02:ab:01:23:45:67"),
            Some([2, 171, 1, 35, 69, 103])
        );
        assert_eq!(parse_mac("02:ab:01"), None);
        assert_eq!(parse_mac("xx:ab:01:23:45:67"), None);
    }
}
