//! conduction-app — Tauri アプリケーション本体。
//!
//! 起動時に audio エンジンスレッドを立ち上げ、Tauri の State として共有する。

#![forbid(unsafe_code)]

pub mod audio_engine;
pub mod commands;
pub mod http_api;
pub mod library_state;
pub mod settings;
pub mod system_stats;
pub mod youtube;

use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

/// アプリのエントリポイント。`main.rs` から呼ばれる。
pub fn run() {
    init_tracing();

    let settings = settings::SettingsHandle::open_default().expect("settings must open");
    let main_device_name = settings.get().audio_main_output.clone();
    let cue_device_name = settings.get().audio_cue_output.clone();
    let audio = audio_engine::spawn(main_device_name, cue_device_name)
        .expect("audio engine must start");
    let library = library_state::LibraryHandle::open_default().expect("library must open");
    let stats = system_stats::SystemStatsHandle::new();
    info!("conduction-app booting");

    // localhost WebAPI を別スレッドで起動。Tauri と同じ State インスタンスを共有する。
    http_api::spawn(
        http_api::AppState {
            audio: audio.clone(),
            library: library.clone(),
            settings: settings.clone(),
            stats: stats.clone(),
        },
        http_api::DEFAULT_HTTP_PORT,
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(audio)
        .manage(library)
        .manage(stats)
        .manage(settings)
        .invoke_handler(tauri::generate_handler![
            commands::load_track,
            commands::play,
            commands::pause,
            commands::stop,
            commands::seek_deck,
            commands::loop_in,
            commands::loop_out,
            commands::loop_toggle,
            commands::loop_clear,
            commands::set_eq,
            commands::set_filter,
            commands::set_echo,
            commands::set_reverb,
            commands::set_cue_send,
            commands::list_audio_devices,
            commands::set_crossfader,
            commands::set_channel_volume,
            commands::set_master_volume,
            commands::set_tempo_adjust,
            commands::set_tempo_range,
            commands::get_status,
            commands::import_track,
            commands::list_tracks,
            commands::delete_track,
            commands::analyze_track,
            commands::get_waveform,
            commands::get_track_beats,
            commands::get_resource_stats,
            commands::get_settings,
            commands::save_settings,
            commands::list_hot_cues,
            commands::set_hot_cue,
            commands::delete_hot_cue,
            commands::yt_dlp_available,
            commands::yt_search,
            commands::yt_download,
            commands::export_preview,
            commands::export_execute,
            commands::insert_cue,
            commands::list_cues,
            commands::delete_cue,
            commands::list_match_candidates,
            commands::inject_demo_cues,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_tracing() {
    // RUST_LOG が未指定の時のデフォルト：
    // - conduction* は debug
    // - Symphonia の MP3 デコーダは seek 直後に "invalid main_data_begin" の WARN を
    //   ビットリザーバ参照解消の都合で出すが、実害がないので error 以上に絞る
    // - その他は info
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(
            "info,\
             conduction_app=debug,conduction_analysis=debug,conduction_library=debug,\
             symphonia=error,symphonia_bundle_mp3=error,symphonia_core=error",
        )
    });
    let _ = fmt().with_env_filter(filter).with_target(true).try_init();
}
