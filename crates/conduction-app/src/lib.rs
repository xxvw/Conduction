//! conduction-app — Tauri アプリケーション本体。
//!
//! 起動時に audio エンジンスレッドを立ち上げ、Tauri の State として共有する。

#![forbid(unsafe_code)]

pub mod audio_engine;
pub mod commands;
pub mod library_state;

use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

/// アプリのエントリポイント。`main.rs` から呼ばれる。
pub fn run() {
    init_tracing();

    let audio = audio_engine::spawn().expect("audio engine must start");
    let library = library_state::LibraryHandle::open_default().expect("library must open");
    info!("conduction-app booting");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(audio)
        .manage(library)
        .invoke_handler(tauri::generate_handler![
            commands::load_track,
            commands::play,
            commands::pause,
            commands::stop,
            commands::set_crossfader,
            commands::set_channel_volume,
            commands::set_master_volume,
            commands::set_tempo_adjust,
            commands::set_tempo_range,
            commands::get_status,
            commands::import_track,
            commands::list_tracks,
            commands::delete_track,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,conduction=debug"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}
