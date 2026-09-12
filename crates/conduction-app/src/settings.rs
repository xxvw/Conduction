//! ユーザー設定の TOML 永続化。
//!
//! 保存先（要件定義書 §13）:
//!   macOS:   `~/Library/Application Support/com.xxvw.conduction/settings.toml`
//!   Windows: `%APPDATA%\Conduction\settings.toml`
//!   Linux:   XDG 規約 (`$XDG_DATA_HOME/Conduction/settings.toml`)
//!
//! 保存はコマンド単位で同期書き込み。frequency が低いので簡素な実装で良い。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use conduction_audio::AudioOutputConfig;
use conduction_link::LinkConfig;
use conduction_midi::MidiConfig;
use directories::ProjectDirs;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// 永続化される全設定。フィールドは必要に応じて拡張する。
#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(default)]
pub struct AppSettings {
    pub keybindings: Vec<KeybindingEntry>,
    /// Main 出力デバイス名（cpal）。`None` の時は OS のデフォルトを使う。
    pub audio_main_output: Option<String>,
    /// Cue 出力デバイス名（cpal）。`None` の時は Cue 出力を使わない。
    pub audio_cue_output: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub audio: Option<AudioOutputConfig>,
    #[schema(value_type = Object)]
    pub link: LinkConfig,
    #[schema(value_type = Vec<Object>)]
    pub midi: Vec<MidiConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LegacySettingsIntent {
    Main,
    Cue,
    Keybindings,
}

impl AppSettings {
    /// Legacy device-only settings become an internal routing configuration.
    pub fn audio_config(&self) -> AudioOutputConfig {
        self.audio.clone().unwrap_or_else(|| AudioOutputConfig {
            device_name: self.audio_main_output.clone(),
            cue_device_name: self.audio_cue_output.clone(),
            cue_pair: self.audio_cue_output.as_ref().map(|_| 0),
            ..Default::default()
        })
    }

    /// Translate a legacy device selector edit without replacing active routing.
    pub fn legacy_audio_update(
        &self,
        incoming: &AppSettings,
        intent: Option<LegacySettingsIntent>,
        active: &AudioOutputConfig,
    ) -> Option<AudioOutputConfig> {
        let main_changed = matches!(intent, None | Some(LegacySettingsIntent::Main))
            && incoming.audio_main_output != self.audio_main_output;
        let cue_changed = matches!(intent, None | Some(LegacySettingsIntent::Cue))
            && incoming.audio_cue_output != self.audio_cue_output;
        if !main_changed && !cue_changed {
            return None;
        }
        let mut config = active.clone();
        if main_changed {
            config.device_name = incoming.audio_main_output.clone();
        }
        if cue_changed {
            config.cue_device_name = incoming.audio_cue_output.clone();
            config.cue_pair = incoming.audio_cue_output.as_ref().map(|_| 0);
        }
        Some(config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct KeybindingEntry {
    pub action: String,
    pub key: String,
    pub label: String,
}

#[derive(Clone)]
pub struct SettingsHandle {
    state: Arc<Mutex<State>>,
}

struct State {
    settings: AppSettings,
    path: PathBuf,
}

impl SettingsHandle {
    pub fn open_default() -> anyhow::Result<Self> {
        let dirs = ProjectDirs::from("com", "xxvw", "Conduction")
            .ok_or_else(|| anyhow::anyhow!("no user data directory available from OS"))?;
        let dir = dirs.data_dir().to_path_buf();
        fs::create_dir_all(&dir)?;
        Ok(Self::open_at(dir.join("settings.toml")))
    }

    fn open_at(path: PathBuf) -> Self {
        let settings = match fs::read_to_string(&path) {
            Ok(s) => match toml::from_str::<AppSettings>(&s) {
                Ok(parsed) => {
                    info!(path = %path.display(), "settings loaded");
                    parsed
                }
                Err(e) => {
                    warn!(error = %e, path = %path.display(), "failed to parse settings, using defaults");
                    AppSettings::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                info!(path = %path.display(), "no settings file yet; starting with defaults");
                AppSettings::default()
            }
            Err(e) => {
                warn!(error = %e, path = %path.display(), "failed to read settings, using defaults");
                AppSettings::default()
            }
        };

        Self {
            state: Arc::new(Mutex::new(State { settings, path })),
        }
    }

    pub fn get(&self) -> AppSettings {
        self.state.lock().settings.clone()
    }

    pub fn set(&self, new: AppSettings) -> anyhow::Result<()> {
        self.update(|current| {
            // Device edits must go through the performance service's audio guard.
            current.keybindings = new.keybindings;
        })
    }

    /// Persist an atomic patch so independent control panels cannot lose edits.
    pub fn update(&self, patch: impl FnOnce(&mut AppSettings)) -> anyhow::Result<()> {
        let mut s = self.state.lock();
        let mut new = s.settings.clone();
        patch(&mut new);
        let body = toml::to_string_pretty(&new)?;
        let temporary = s.path.with_extension("toml.tmp");
        fs::write(&temporary, body)?;
        fs::rename(temporary, &s.path)?;
        info!(path = %s.path.display(), "settings saved");
        s.settings = new;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduction_audio::MixingMode;
    use conduction_midi::{Binding, Control, InputMode, MidiMessage, RelativeEncoding};
    use std::net::Ipv4Addr;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestSettings {
        directory: PathBuf,
        handle: SettingsHandle,
    }

    impl TestSettings {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!(
                "conduction-settings-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&directory).unwrap();
            let handle = SettingsHandle::open_at(directory.join("settings.toml"));
            Self { directory, handle }
        }

        fn path(&self) -> PathBuf {
            self.directory.join("settings.toml")
        }

        fn reload(&self) -> AppSettings {
            SettingsHandle::open_at(self.path()).get()
        }
    }

    impl Drop for TestSettings {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    fn connection_settings() -> AppSettings {
        AppSettings {
            audio_main_output: Some("DJM-A9".into()),
            audio_cue_output: Some("Monitor".into()),
            audio: Some(AudioOutputConfig {
                device_name: Some("DJM-A9".into()),
                cue_device_name: Some("Monitor".into()),
                sample_rate: Some(96_000),
                buffer_frames: Some(512),
                mode: MixingMode::External,
                main_pair: 0,
                cue_pair: Some(2),
                deck_a_pair: 4,
                deck_b_pair: 6,
                headphone_mix: 0.25,
                headphone_volume: 0.75,
            }),
            link: LinkConfig {
                enabled: true,
                interface_ip: Ipv4Addr::new(192, 168, 10, 2),
                broadcast_ip: Ipv4Addr::new(192, 168, 10, 255),
                mac_address: [2, 17, 34, 51, 68, 85],
                source_deck: "B".into(),
                latency_ms: 8.5,
                library_enabled: true,
                preferred_player: Some(3),
            },
            midi: vec![
                MidiConfig {
                    input_port: "DDJ-FLX10 MIDI IN".into(),
                    output_port: Some("DDJ-FLX10 MIDI OUT".into()),
                    profile: "ddj-flx10".into(),
                    deck: None,
                    bindings: vec![Binding {
                        message: MidiMessage::ControlChange {
                            channel: 0,
                            number: 7,
                        },
                        control: Control::Fader { deck: "A".into() },
                        mode: InputMode::Absolute14 { lsb: 39 },
                        pickup: true,
                    }],
                },
                MidiConfig {
                    input_port: "CDJ-3000".into(),
                    output_port: None,
                    profile: "cdj-3000".into(),
                    deck: Some("B".into()),
                    bindings: vec![Binding {
                        message: MidiMessage::ControlChange {
                            channel: 1,
                            number: 33,
                        },
                        control: Control::Jog { deck: "B".into() },
                        mode: InputMode::Relative {
                            encoding: RelativeEncoding::Offset64,
                            step: 0.125,
                        },
                        pickup: false,
                    }],
                },
            ],
            keybindings: vec![KeybindingEntry {
                action: "play_a".into(),
                key: "Space".into(),
                label: "Play deck A".into(),
            }],
        }
    }

    fn assert_settings_eq(actual: &AppSettings, expected: &AppSettings) {
        assert_eq!(
            toml::to_string_pretty(actual).unwrap(),
            toml::to_string_pretty(expected).unwrap()
        );
    }

    #[test]
    fn legacy_device_only_settings_default_to_internal_with_link_disabled() {
        let fixture = TestSettings::new();
        fs::write(
            fixture.path(),
            "audio_main_output = 'Main USB'\naudio_cue_output = 'Cue USB'\n",
        )
        .unwrap();

        let settings = fixture.reload();
        assert!(settings.audio.is_none());
        assert_eq!(
            settings.audio_config(),
            AudioOutputConfig {
                device_name: Some("Main USB".into()),
                cue_device_name: Some("Cue USB".into()),
                cue_pair: Some(0),
                ..Default::default()
            }
        );
        assert!(!settings.link.enabled);
        assert!(!settings.link.library_enabled);
        assert!(settings.midi.is_empty());
    }

    #[test]
    fn audio_routes_link_and_custom_midi_profiles_roundtrip() {
        let fixture = TestSettings::new();
        let expected = connection_settings();
        fixture.handle.update(|s| *s = expected.clone()).unwrap();

        assert_settings_eq(&fixture.handle.get(), &expected);
        assert_settings_eq(&fixture.reload(), &expected);
    }

    #[test]
    fn stale_settings_set_changes_only_keybindings() {
        let fixture = TestSettings::new();
        let mut stale = fixture.handle.get();
        let mut expected = connection_settings();
        fixture.handle.update(|s| *s = expected.clone()).unwrap();
        stale.keybindings = vec![KeybindingEntry {
            action: "cue_b".into(),
            key: "Q".into(),
            label: "Cue deck B".into(),
        }];
        expected.keybindings = stale.keybindings.clone();

        fixture.handle.set(stale).unwrap();

        assert_settings_eq(&fixture.handle.get(), &expected);
        assert_settings_eq(&fixture.reload(), &expected);
    }

    #[test]
    fn failed_write_preserves_memory_and_original_persisted_settings() {
        let fixture = TestSettings::new();
        let expected = connection_settings();
        fixture.handle.update(|s| *s = expected.clone()).unwrap();
        let original = fs::read(fixture.path()).unwrap();
        fs::create_dir(fixture.path().with_extension("toml.tmp")).unwrap();

        let result = fixture.handle.update(|s| {
            s.keybindings.clear();
            s.audio = None;
            s.link.enabled = false;
            s.midi.clear();
        });

        assert!(result.is_err());
        assert_settings_eq(&fixture.handle.get(), &expected);
        assert_settings_eq(&fixture.reload(), &expected);
        assert_eq!(fs::read(fixture.path()).unwrap(), original);
    }

    #[test]
    fn omitted_intent_updates_legacy_devices_and_preserves_active_routes() {
        let settings = connection_settings();
        let mut incoming = settings.clone();
        incoming.audio_main_output = Some("Replacement main".into());
        incoming.audio_cue_output = Some("Replacement cue".into());
        incoming.audio = None;
        let mut active = settings.audio_config();
        active.buffer_frames = Some(256);
        active.deck_a_pair = 2;
        let mut expected = active.clone();
        expected.device_name = incoming.audio_main_output.clone();
        expected.cue_device_name = incoming.audio_cue_output.clone();
        expected.cue_pair = Some(0);

        assert_eq!(
            settings.legacy_audio_update(&incoming, None, &active),
            Some(expected)
        );
        assert_eq!(settings.legacy_audio_update(&settings, None, &active), None);
    }

    #[test]
    fn keybinding_intent_ignores_stale_legacy_device_names() {
        let settings = connection_settings();
        assert!(settings
            .legacy_audio_update(
                &AppSettings::default(),
                Some(LegacySettingsIntent::Keybindings),
                &settings.audio_config(),
            )
            .is_none());
    }

    #[test]
    fn explicit_device_intents_change_only_the_selected_device() {
        let settings = connection_settings();
        let incoming = AppSettings::default();
        let active = settings.audio_config();
        let mut expected_main = active.clone();
        expected_main.device_name = None;
        let mut expected_cue = active.clone();
        expected_cue.cue_device_name = None;
        expected_cue.cue_pair = None;

        assert_eq!(
            settings.legacy_audio_update(&incoming, Some(LegacySettingsIntent::Main), &active),
            Some(expected_main)
        );
        assert_eq!(
            settings.legacy_audio_update(&incoming, Some(LegacySettingsIntent::Cue), &active),
            Some(expected_cue)
        );
    }
}
