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

use directories::ProjectDirs;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// 永続化される全設定。フィールドは必要に応じて拡張する。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub keybindings: Vec<KeybindingEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingEntry {
    pub action: String,
    pub key: String,
    pub label: String,
}

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
        let path = dir.join("settings.toml");

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

        Ok(Self {
            state: Arc::new(Mutex::new(State { settings, path })),
        })
    }

    pub fn get(&self) -> AppSettings {
        self.state.lock().settings.clone()
    }

    pub fn set(&self, new: AppSettings) -> anyhow::Result<()> {
        let mut s = self.state.lock();
        let body = toml::to_string_pretty(&new)?;
        fs::write(&s.path, body)?;
        info!(path = %s.path.display(), "settings saved");
        s.settings = new;
        Ok(())
    }
}
