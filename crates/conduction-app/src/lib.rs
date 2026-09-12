//! conduction-app — Tauri アプリケーション本体。
//!
//! 起動時に audio エンジンスレッドを立ち上げ、Tauri の State として共有する。

#![forbid(unsafe_code)]

pub mod audio_engine;
pub mod commands;
pub mod export_state;
pub mod http_api;
pub mod library_state;
pub mod link_library;
pub mod performance;
mod performance_bridge;
mod performance_commands;
mod performance_http;
pub mod setlist_state;
pub mod settings;
pub mod system_stats;
pub mod youtube;

use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

/// アプリのエントリポイント。`main.rs` から呼ばれる。
pub fn run() {
    init_tracing();

    let settings = settings::SettingsHandle::open_default().expect("settings must open");
    let audio = audio_engine::spawn_with_config(settings.get().audio_config())
        .expect("audio engine must start");
    let library = library_state::LibraryHandle::open_default().expect("library must open");
    let performance =
        performance::PerformanceHandle::new(audio.clone(), library.clone(), settings.clone())
            .expect("performance service must start");
    let restore = performance.clone();
    tauri::async_runtime::spawn(async move {
        restore.restore().await;
    });
    let setlists = setlist_state::SetlistHandle::new(library.shared());
    let stats = system_stats::SystemStatsHandle::new();
    let export_registry = {
        let mut r = conduction_export::default_registry();
        r.register_exporter(conduction_rekordbox::RekordboxXmlExporter::new());
        export_state::ExportRegistryHandle::new(r)
    };
    info!("conduction-app booting");

    // localhost WebAPI を別スレッドで起動。Tauri と同じ State インスタンスを共有する。
    http_api::spawn(
        http_api::AppState {
            audio: audio.clone(),
            library: library.clone(),
            settings: settings.clone(),
            stats: stats.clone(),
            setlists: setlists.clone(),
            performance: performance.clone(),
        },
        http_api::DEFAULT_HTTP_PORT,
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(audio)
        .manage(library)
        .manage(stats)
        .manage(settings)
        .manage(setlists)
        .manage(export_registry)
        .manage(performance)
        .invoke_handler(tauri::generate_handler![
            performance_commands::get_performance_status,
            performance_commands::perform,
            performance_commands::list_audio_outputs,
            performance_commands::configure_audio,
            performance_commands::list_link_interfaces,
            performance_commands::configure_link,
            performance_commands::refresh_link_library,
            performance_commands::request_link_master,
            performance_commands::list_midi_devices,
            performance_commands::list_midi_profiles,
            performance_commands::configure_midi,
            performance_commands::disconnect_midi,
            performance_commands::set_performance_browser,
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
            commands::set_key_lock,
            commands::set_pitch_offset,
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
            commands::list_template_presets,
            commands::get_template_preset,
            commands::start_template_preset,
            commands::save_user_template,
            commands::delete_user_template,
            commands::compile_lua_template,
            commands::abort_template,
            commands::override_param,
            commands::resume_param,
            commands::commit_param,
            commands::list_setlists,
            commands::create_setlist,
            commands::delete_setlist,
            commands::rename_setlist,
            commands::setlist_add_entry,
            commands::setlist_remove_entry,
            commands::setlist_move_entry,
            commands::setlist_set_transition,
            commands::setlist_export,
            commands::setlist_import,
            commands::list_export_formats,
            commands::list_import_formats,
            commands::library_export,
            commands::library_import,
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
