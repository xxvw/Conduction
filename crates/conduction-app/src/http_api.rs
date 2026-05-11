//! Conduction の全パラメータを操作する localhost WebAPI。
//!
//! `127.0.0.1:38127` に bind し、認証なしで動作する。
//! Tauri 内部の `AudioHandle` / `LibraryHandle` / `SettingsHandle` /
//! `SystemStatsHandle` をそのまま参照するので、UI 経由の操作と完全に等価。
//!
//! - `GET  /api/health`            — 死活
//! - `GET  /api/status`            — Mixer + Decks のスナップショット
//! - `GET  /api/audio-devices`     — 接続中の出力デバイス名一覧
//! - `GET  /api/resources`         — CPU/メモリ統計
//! - `GET  /api/settings`          — 永続設定
//! - `PUT  /api/settings`          — 永続設定の上書き
//! - `GET  /api/tracks`            — ライブラリのトラック一覧
//! - `POST /api/tracks/import`     — 任意 path をインポート
//! - `DELETE /api/tracks/{id}`     — 削除
//! - `POST /api/tracks/{id}/analyze`     — 解析実行
//! - `GET  /api/tracks/{id}/waveform`    — 解析済波形
//! - `GET  /api/tracks/{id}/beats`       — ビートグリッド
//! - `GET  /api/tracks/{id}/hotcues`     — Hot Cue 一覧
//! - `PUT  /api/tracks/{id}/hotcues/{slot}` — Hot Cue の保存
//! - `DELETE /api/tracks/{id}/hotcues/{slot}` — 削除
//! - `POST /api/decks/{id}/...`    — 各種 Deck 操作 (load / play / pause / ...)
//! - `POST /api/mixer/...`         — Crossfader / Master Volume
//!
//! Swagger UI は `/swagger-ui` でホストする。OpenAPI JSON は `/api-docs/openapi.json`。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use conduction_analysis::{
    decode_to_pcm, estimate_beatgrid, estimate_key, generate_waveform, WaveformPreview,
    DEFAULT_WAVEFORM_BINS,
};
use conduction_audio::OutputDevice;
use conduction_core::TrackId;
use conduction_library::{build_track_from_file, Library};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

use crate::audio_engine::{parse_deck, parse_tempo_range, AudioCommand, AudioHandle, MixerSnapshot};
use crate::commands::{
    BeatDto, CueDto, HotCueDto, InsertCueArgs, MatchCandidateDto, MatchQueryArgs, TemplatePresetDto,
};
use crate::library_state::{LibraryHandle, TrackSummary};
use crate::settings::{AppSettings, KeybindingEntry, SettingsHandle};
use crate::setlist_state::SetlistHandle;
use crate::system_stats::{ResourceStats, SystemStatsHandle};
use crate::youtube;
use conduction_download::{AudioFormat, VideoSearchResult};
use conduction_export::{ExportPreview, ExportReport};

pub const DEFAULT_HTTP_PORT: u16 = 38127;

#[derive(Clone)]
pub struct AppState {
    pub audio: AudioHandle,
    pub library: LibraryHandle,
    pub settings: SettingsHandle,
    pub stats: SystemStatsHandle,
    pub setlists: SetlistHandle,
}

/// API 起動。専用スレッドで tokio runtime を立ててから axum を block_on する。
pub fn spawn(state: AppState, port: u16) {
    thread::Builder::new()
        .name("conduction-http".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .thread_name("conduction-http-worker")
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    error!(?e, "failed to start http runtime");
                    return;
                }
            };
            rt.block_on(async move {
                if let Err(e) = serve(state, port).await {
                    error!(?e, "http api server stopped with error");
                }
            });
        })
        .expect("failed to spawn http-api thread");
}

async fn serve(state: AppState, port: u16) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("http api listening on http://{addr} (swagger: http://{addr}/swagger-ui)");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    let api = Router::new()
        .route("/api/health", get(health))
        .route("/api/status", get(get_status))
        .route("/api/audio-devices", get(list_audio_devices))
        .route("/api/resources", get(get_resources))
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/tracks", get(list_tracks))
        .route("/api/tracks/import", post(import_track))
        .route("/api/tracks/:id", delete(delete_track))
        .route("/api/tracks/:id/analyze", post(analyze_track))
        .route("/api/tracks/:id/waveform", get(get_waveform))
        .route("/api/tracks/:id/beats", get(get_beats))
        .route("/api/tracks/:id/hotcues", get(list_hot_cues))
        .route(
            "/api/tracks/:id/hotcues/:slot",
            put(set_hot_cue).delete(delete_hot_cue),
        )
        .route("/api/decks/:id/load", post(load_track))
        .route("/api/decks/:id/play", post(play))
        .route("/api/decks/:id/pause", post(pause))
        .route("/api/decks/:id/stop", post(stop))
        .route("/api/decks/:id/seek", post(seek_deck))
        .route("/api/decks/:id/loop/in", post(loop_in))
        .route("/api/decks/:id/loop/out", post(loop_out))
        .route("/api/decks/:id/loop/toggle", post(loop_toggle))
        .route("/api/decks/:id/loop/clear", post(loop_clear))
        .route("/api/decks/:id/eq", post(set_eq))
        .route("/api/decks/:id/filter", post(set_filter))
        .route("/api/decks/:id/echo", post(set_echo))
        .route("/api/decks/:id/reverb", post(set_reverb))
        .route("/api/decks/:id/cue-send", post(set_cue_send))
        .route("/api/decks/:id/key-lock", post(set_key_lock))
        .route("/api/decks/:id/pitch-offset", post(set_pitch_offset))
        .route("/api/decks/:id/channel-volume", post(set_channel_volume))
        .route("/api/decks/:id/tempo-adjust", post(set_tempo_adjust))
        .route("/api/decks/:id/tempo-range", post(set_tempo_range))
        .route("/api/mixer/crossfader", post(set_crossfader))
        .route("/api/mixer/master-volume", post(set_master_volume))
        .route("/api/youtube/available", get(yt_available))
        .route("/api/youtube/search", get(yt_search))
        .route("/api/youtube/download", post(yt_download))
        .route("/api/export/preview", post(export_preview))
        .route("/api/export/execute", post(export_execute))
        .route("/api/tracks/:id/cues", get(list_cues_for_track).post(insert_cue))
        .route("/api/cues/:id", delete(delete_cue))
        .route("/api/match", post(list_match_candidates))
        .route("/api/templates/presets", get(list_template_presets))
        .route("/api/templates/presets/:id", get(get_template_preset))
        .route("/api/templates/start", post(start_template_preset))
        .route("/api/templates/abort", post(abort_template))
        .route("/api/templates/override", post(override_param))
        .route("/api/templates/resume", post(resume_param))
        .route("/api/templates/commit", post(commit_param))
        .route("/api/setlists", get(list_setlists).post(create_setlist))
        .route("/api/setlists/:id", delete(delete_setlist))
        .route(
            "/api/setlists/:id/entries",
            post(setlist_add_entry),
        )
        .route(
            "/api/setlists/:id/entries/:entry_id",
            delete(setlist_remove_entry),
        )
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(api)
}

// ============================================================================
// Handlers
// ============================================================================

#[derive(Debug)]
struct ApiError(StatusCode, String);

impl ApiError {
    fn bad_request<S: Into<String>>(msg: S) -> Self {
        Self(StatusCode::BAD_REQUEST, msg.into())
    }
    fn internal<S: Into<String>>(msg: S) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, msg.into())
    }
    fn not_found<S: Into<String>>(msg: S) -> Self {
        Self(StatusCode::NOT_FOUND, msg.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({ "error": self.1 });
        (self.0, Json(body)).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

fn parse_track_id(s: &str) -> ApiResult<TrackId> {
    let uuid = Uuid::parse_str(s).map_err(|e| ApiError::bad_request(format!("invalid track id: {e}")))?;
    Ok(TrackId::from_uuid(uuid))
}

fn send_audio(audio: &AudioHandle, cmd: AudioCommand) -> ApiResult<()> {
    audio.send(cmd).map_err(|e| ApiError::internal(e.to_string()))
}

#[utoipa::path(get, path = "/api/health", responses((status = 200, body = HealthResponse)))]
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

#[utoipa::path(get, path = "/api/status", responses((status = 200, body = MixerSnapshot)))]
async fn get_status(State(s): State<AppState>) -> Json<MixerSnapshot> {
    Json(s.audio.snapshot())
}

#[utoipa::path(get, path = "/api/audio-devices", responses((status = 200, body = Vec<String>)))]
async fn list_audio_devices() -> Json<Vec<String>> {
    Json(OutputDevice::list_available())
}

#[utoipa::path(get, path = "/api/resources", responses((status = 200, body = ResourceStats)))]
async fn get_resources(State(s): State<AppState>) -> Json<ResourceStats> {
    Json(s.stats.snapshot())
}

#[utoipa::path(get, path = "/api/settings", responses((status = 200, body = AppSettings)))]
async fn get_settings(State(s): State<AppState>) -> Json<AppSettings> {
    Json(s.settings.get())
}

#[utoipa::path(put, path = "/api/settings", request_body = AppSettings, responses((status = 200)))]
async fn put_settings(
    State(s): State<AppState>,
    Json(new_settings): Json<AppSettings>,
) -> ApiResult<StatusCode> {
    s.settings
        .set(new_settings)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::OK)
}

// ---- Tracks --------------------------------------------------------------

#[utoipa::path(get, path = "/api/tracks", responses((status = 200, body = Vec<TrackSummary>)))]
async fn list_tracks(State(s): State<AppState>) -> ApiResult<Json<Vec<TrackSummary>>> {
    let v = s
        .library
        .with_library(|lib| {
            lib.list_tracks()
                .map(|tracks| tracks.iter().map(TrackSummary::from_track).collect::<Vec<_>>())
                .map_err(|e| e.to_string())
        })
        .map_err(ApiError::internal)?;
    Ok(Json(v))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ImportTrackRequest {
    pub path: String,
}

#[utoipa::path(post, path = "/api/tracks/import", request_body = ImportTrackRequest, responses((status = 200, body = TrackSummary)))]
async fn import_track(
    State(s): State<AppState>,
    Json(body): Json<ImportTrackRequest>,
) -> ApiResult<Json<TrackSummary>> {
    info!(path = %body.path, "http import_track");
    let path_buf = PathBuf::from(&body.path);
    let track = build_track_from_file(&path_buf).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let stored = s
        .library
        .with_library(|lib| -> Result<_, String> {
            let id = lib.upsert_track_by_path(&track).map_err(|e| e.to_string())?;
            let stored = lib
                .get_track(id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "track disappeared after insert".to_string())?;
            Ok(stored)
        })
        .map_err(ApiError::internal)?;

    // 既存波形が無ければバックグラウンド解析を起動。
    let needs_waveform = s
        .library
        .with_library(|lib| lib.load_waveform(stored.id).map(|w| w.is_none()))
        .unwrap_or(true);
    if needs_waveform {
        let lib_shared = s.library.shared();
        let track_id = stored.id;
        let analyze_path = stored.path.clone();
        let _ = thread::Builder::new()
            .name("analyze-import-http".into())
            .spawn(move || {
                if let Err(e) = analyze_and_save(&lib_shared, track_id, &analyze_path) {
                    warn!(error = %e, "background analyze (http) failed");
                }
            });
    }
    Ok(Json(TrackSummary::from_track(&stored)))
}

#[utoipa::path(delete, path = "/api/tracks/{id}", responses((status = 200)))]
async fn delete_track(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let tid = parse_track_id(&id)?;
    s.library
        .with_library(|lib| lib.delete_track(tid).map_err(|e| e.to_string()))
        .map_err(ApiError::internal)?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/tracks/{id}/analyze", responses((status = 200, body = Object)))]
async fn analyze_track(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<WaveformPreview>> {
    let tid = parse_track_id(&id)?;
    let path = s
        .library
        .with_library(|lib| -> Result<PathBuf, String> {
            let t = lib
                .get_track(tid)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("track not found: {id}"))?;
            Ok(t.path)
        })
        .map_err(ApiError::not_found)?;
    let wf = analyze_and_save(&s.library.shared(), tid, &path)
        .map_err(ApiError::internal)?;
    Ok(Json(wf))
}

#[utoipa::path(get, path = "/api/tracks/{id}/waveform", responses((status = 200, body = Object)))]
async fn get_waveform(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Option<WaveformPreview>>> {
    let tid = parse_track_id(&id)?;
    let result = s
        .library
        .with_library(|lib| lib.load_waveform(tid).map_err(|e| e.to_string()))
        .map_err(ApiError::internal)?;
    Ok(Json(result))
}

#[utoipa::path(get, path = "/api/tracks/{id}/beats", responses((status = 200, body = Vec<BeatDto>)))]
async fn get_beats(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<BeatDto>>> {
    let tid = parse_track_id(&id)?;
    let beats = s
        .library
        .with_library(|lib| {
            lib.load_beatgrid(tid)
                .map(|b| b.iter().map(BeatDto::from).collect::<Vec<_>>())
                .map_err(|e| e.to_string())
        })
        .map_err(ApiError::internal)?;
    Ok(Json(beats))
}

#[utoipa::path(get, path = "/api/tracks/{id}/hotcues", responses((status = 200, body = Vec<HotCueDto>)))]
async fn list_hot_cues(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<HotCueDto>>> {
    let tid = parse_track_id(&id)?;
    let rows = s
        .library
        .with_library(|lib| {
            lib.list_hot_cues(tid)
                .map(|rows| {
                    rows.into_iter()
                        .map(|(slot, position_sec)| HotCueDto { slot, position_sec })
                        .collect::<Vec<_>>()
                })
                .map_err(|e| e.to_string())
        })
        .map_err(ApiError::internal)?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SetHotCueRequest {
    pub position_sec: f64,
}

#[utoipa::path(put, path = "/api/tracks/{id}/hotcues/{slot}", request_body = SetHotCueRequest, responses((status = 200)))]
async fn set_hot_cue(
    State(s): State<AppState>,
    Path((id, slot)): Path<(String, u8)>,
    Json(body): Json<SetHotCueRequest>,
) -> ApiResult<StatusCode> {
    let tid = parse_track_id(&id)?;
    s.library
        .with_library(|lib| lib.set_hot_cue(tid, slot, body.position_sec).map_err(|e| e.to_string()))
        .map_err(ApiError::internal)?;
    Ok(StatusCode::OK)
}

#[utoipa::path(delete, path = "/api/tracks/{id}/hotcues/{slot}", responses((status = 200)))]
async fn delete_hot_cue(
    State(s): State<AppState>,
    Path((id, slot)): Path<(String, u8)>,
) -> ApiResult<StatusCode> {
    let tid = parse_track_id(&id)?;
    s.library
        .with_library(|lib| lib.delete_hot_cue(tid, slot).map_err(|e| e.to_string()))
        .map_err(ApiError::internal)?;
    Ok(StatusCode::OK)
}

// ---- Deck operations -----------------------------------------------------

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct LoadTrackRequest {
    pub path: String,
}

#[utoipa::path(post, path = "/api/decks/{id}/load", request_body = LoadTrackRequest, responses((status = 200)))]
async fn load_track(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<LoadTrackRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(
        &s.audio,
        AudioCommand::Load {
            deck,
            path: PathBuf::from(body.path),
        },
    )?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/decks/{id}/play", responses((status = 200)))]
async fn play(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::Play(deck))?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/decks/{id}/pause", responses((status = 200)))]
async fn pause(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::Pause(deck))?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/decks/{id}/stop", responses((status = 200)))]
async fn stop(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::Stop(deck))?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SeekRequest {
    pub position_sec: f64,
}

#[utoipa::path(post, path = "/api/decks/{id}/seek", request_body = SeekRequest, responses((status = 200)))]
async fn seek_deck(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SeekRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(
        &s.audio,
        AudioCommand::Seek {
            deck,
            position_sec: body.position_sec,
        },
    )?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct PositionRequest {
    pub position_sec: f64,
}

#[utoipa::path(post, path = "/api/decks/{id}/loop/in", request_body = PositionRequest, responses((status = 200)))]
async fn loop_in(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PositionRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(
        &s.audio,
        AudioCommand::LoopIn {
            deck,
            position_sec: body.position_sec,
        },
    )?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/decks/{id}/loop/out", request_body = PositionRequest, responses((status = 200)))]
async fn loop_out(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PositionRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(
        &s.audio,
        AudioCommand::LoopOut {
            deck,
            position_sec: body.position_sec,
        },
    )?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/decks/{id}/loop/toggle", responses((status = 200)))]
async fn loop_toggle(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::LoopToggle(deck))?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/decks/{id}/loop/clear", responses((status = 200)))]
async fn loop_clear(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::LoopClear(deck))?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct EqRequest {
    /// "low" | "mid" | "high"
    pub band: String,
    pub db: f32,
}

#[utoipa::path(post, path = "/api/decks/{id}/eq", request_body = EqRequest, responses((status = 200)))]
async fn set_eq(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<EqRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    let cmd = match body.band.as_str() {
        "low" | "Low" => AudioCommand::SetEqLow { deck, db: body.db },
        "mid" | "Mid" => AudioCommand::SetEqMid { deck, db: body.db },
        "high" | "High" => AudioCommand::SetEqHigh { deck, db: body.db },
        other => return Err(ApiError::bad_request(format!("invalid eq band: {other}"))),
    };
    send_audio(&s.audio, cmd)?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ScalarRequest {
    pub value: f32,
}

#[utoipa::path(post, path = "/api/decks/{id}/filter", request_body = ScalarRequest, responses((status = 200)))]
async fn set_filter(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ScalarRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::SetFilter { deck, value: body.value })?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct EchoRequest {
    pub wet: f32,
    pub time_ms: f32,
    pub feedback: f32,
}

#[utoipa::path(post, path = "/api/decks/{id}/echo", request_body = EchoRequest, responses((status = 200)))]
async fn set_echo(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<EchoRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(
        &s.audio,
        AudioCommand::SetEcho {
            deck,
            wet: body.wet,
            time_ms: body.time_ms,
            feedback: body.feedback,
        },
    )?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ReverbRequest {
    pub wet: f32,
    pub room: f32,
}

#[utoipa::path(post, path = "/api/decks/{id}/reverb", request_body = ReverbRequest, responses((status = 200)))]
async fn set_reverb(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ReverbRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(
        &s.audio,
        AudioCommand::SetReverb {
            deck,
            wet: body.wet,
            room: body.room,
        },
    )?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/decks/{id}/cue-send", request_body = ScalarRequest, responses((status = 200)))]
async fn set_cue_send(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ScalarRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::SetCueSend { deck, value: body.value })?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct KeyLockRequest {
    pub on: bool,
}

#[utoipa::path(post, path = "/api/decks/{id}/key-lock", request_body = KeyLockRequest, responses((status = 200)))]
async fn set_key_lock(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<KeyLockRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::SetKeyLock { deck, on: body.on })?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct PitchOffsetRequest {
    pub semitones: f32,
}

#[utoipa::path(post, path = "/api/decks/{id}/pitch-offset", request_body = PitchOffsetRequest, responses((status = 200)))]
async fn set_pitch_offset(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PitchOffsetRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(
        &s.audio,
        AudioCommand::SetPitchOffset {
            deck,
            semitones: body.semitones,
        },
    )?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct VolumeRequest {
    pub volume: f32,
}

#[utoipa::path(post, path = "/api/decks/{id}/channel-volume", request_body = VolumeRequest, responses((status = 200)))]
async fn set_channel_volume(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<VolumeRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(
        &s.audio,
        AudioCommand::SetChannelVolume {
            deck,
            volume: body.volume,
        },
    )?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct TempoAdjustRequest {
    pub adjust: f32,
}

#[utoipa::path(post, path = "/api/decks/{id}/tempo-adjust", request_body = TempoAdjustRequest, responses((status = 200)))]
async fn set_tempo_adjust(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TempoAdjustRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    send_audio(
        &s.audio,
        AudioCommand::SetTempoAdjust {
            deck,
            adjust: body.adjust,
        },
    )?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct TempoRangeRequest {
    /// 6, 10, 16
    pub percent: u8,
}

#[utoipa::path(post, path = "/api/decks/{id}/tempo-range", request_body = TempoRangeRequest, responses((status = 200)))]
async fn set_tempo_range(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<TempoRangeRequest>,
) -> ApiResult<StatusCode> {
    let deck = parse_deck(&id).map_err(ApiError::bad_request)?;
    let range = parse_tempo_range(body.percent).map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::SetTempoRange { deck, range })?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CrossfaderRequest {
    pub position: f32,
}

#[utoipa::path(post, path = "/api/mixer/crossfader", request_body = CrossfaderRequest, responses((status = 200)))]
async fn set_crossfader(
    State(s): State<AppState>,
    Json(body): Json<CrossfaderRequest>,
) -> ApiResult<StatusCode> {
    send_audio(&s.audio, AudioCommand::SetCrossfader(body.position))?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/mixer/master-volume", request_body = VolumeRequest, responses((status = 200)))]
async fn set_master_volume(
    State(s): State<AppState>,
    Json(body): Json<VolumeRequest>,
) -> ApiResult<StatusCode> {
    send_audio(&s.audio, AudioCommand::SetMasterVolume(body.volume))?;
    Ok(StatusCode::OK)
}

// ---- YouTube ------------------------------------------------------------

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct YtAvailableResponse {
    pub available: bool,
}

#[utoipa::path(get, path = "/api/youtube/available", responses((status = 200, body = YtAvailableResponse)))]
async fn yt_available() -> Json<YtAvailableResponse> {
    Json(YtAvailableResponse {
        available: youtube::is_available(),
    })
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct YtSearchQuery {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_limit() -> u32 {
    10
}

#[utoipa::path(
    get,
    path = "/api/youtube/search",
    params(YtSearchQuery),
    responses((status = 200, body = Vec<VideoSearchResult>))
)]
async fn yt_search(
    axum::extract::Query(q): axum::extract::Query<YtSearchQuery>,
) -> ApiResult<Json<Vec<VideoSearchResult>>> {
    let results =
        youtube::search(&q.q, q.limit as usize).map_err(ApiError::internal)?;
    Ok(Json(results))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct YtDownloadRequest {
    pub url: String,
    /// "m4a" | "mp3" | "opus" | "wav" | "flac"
    pub format: String,
}

#[utoipa::path(
    post,
    path = "/api/youtube/download",
    request_body = YtDownloadRequest,
    responses((status = 200, body = TrackSummary))
)]
async fn yt_download(
    State(s): State<AppState>,
    Json(body): Json<YtDownloadRequest>,
) -> ApiResult<Json<TrackSummary>> {
    let format = youtube::parse_format(&body.format).map_err(ApiError::bad_request)?;
    let library = s.library.clone();
    let url = body.url.clone();
    // 同期 spawn (yt-dlp が blocking) を tokio から逃がす。HTTP API では進捗 stream は無し。
    let track = tokio::task::spawn_blocking(move || {
        youtube::download_and_import(&library, &url, format, |_| {})
    })
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .map_err(ApiError::internal)?;
    Ok(Json(track))
}

// ---- Export (rekordbox-compatible USB) ---------------------------------

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ExportRequest {
    /// USB ドライブ等の絶対パス。
    pub destination: String,
}

#[utoipa::path(
    post,
    path = "/api/export/preview",
    request_body = ExportRequest,
    responses((status = 200, body = ExportPreview))
)]
async fn export_preview(
    State(s): State<AppState>,
    Json(body): Json<ExportRequest>,
) -> ApiResult<Json<ExportPreview>> {
    let dest = std::path::PathBuf::from(body.destination);
    let library = s.library.clone();
    let preview = tokio::task::spawn_blocking(move || -> Result<ExportPreview, String> {
        let plan = library
            .with_library(|lib| conduction_export::build_plan(lib, dest).map_err(|e| e.to_string()))?;
        Ok(ExportPreview::from_plan(&plan))
    })
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .map_err(ApiError::internal)?;
    Ok(Json(preview))
}

#[utoipa::path(
    post,
    path = "/api/export/execute",
    request_body = ExportRequest,
    responses((status = 200, body = ExportReport))
)]
async fn export_execute(
    State(s): State<AppState>,
    Json(body): Json<ExportRequest>,
) -> ApiResult<Json<ExportReport>> {
    let dest = std::path::PathBuf::from(body.destination);
    let library = s.library.clone();
    let report = tokio::task::spawn_blocking(move || -> Result<ExportReport, String> {
        let plan = library
            .with_library(|lib| conduction_export::build_plan(lib, dest).map_err(|e| e.to_string()))?;
        conduction_export::execute(&plan).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .map_err(ApiError::internal)?;
    Ok(Json(report))
}

// ---- Typed Cue ----------------------------------------------------------

#[utoipa::path(
    get,
    path = "/api/tracks/{id}/cues",
    responses((status = 200, body = Vec<CueDto>))
)]
async fn list_cues_for_track(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<CueDto>>> {
    let tid = parse_track_id(&id)?;
    let cues = s
        .library
        .with_library(|lib| {
            lib.list_cues_for_track(tid)
                .map(|cs| cs.iter().map(CueDto::from).collect::<Vec<_>>())
                .map_err(|e| e.to_string())
        })
        .map_err(ApiError::internal)?;
    Ok(Json(cues))
}

#[utoipa::path(
    post,
    path = "/api/tracks/{id}/cues",
    request_body = InsertCueArgs,
    responses((status = 200, body = CueDto))
)]
async fn insert_cue(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(mut body): Json<InsertCueArgs>,
) -> ApiResult<Json<CueDto>> {
    // path id を信頼ソースとして上書き (body と齟齬があっても無視)
    body.track_id = id;
    crate::commands::insert_cue_impl(&s.library, body)
        .map(Json)
        .map_err(ApiError::internal)
}

#[utoipa::path(delete, path = "/api/cues/{id}", responses((status = 200)))]
async fn delete_cue(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let uuid = Uuid::parse_str(&id).map_err(|e| ApiError::bad_request(format!("invalid cue id: {e}")))?;
    let cid = conduction_core::CueId::from_uuid(uuid);
    s.library
        .with_library(|lib| lib.delete_cue(cid).map_err(|e| e.to_string()))
        .map_err(ApiError::internal)?;
    Ok(StatusCode::OK)
}

// ---- Templates ----------------------------------------------------------

#[utoipa::path(get, path = "/api/templates/presets", responses((status = 200, body = Vec<TemplatePresetDto>)))]
async fn list_template_presets(State(s): State<AppState>) -> Json<Vec<TemplatePresetDto>> {
    Json(crate::commands::list_template_presets_impl(&s.library))
}

#[utoipa::path(get, path = "/api/templates/presets/{id}", responses((status = 200, body = Object)))]
async fn get_template_preset(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<conduction_conductor::Template>> {
    crate::commands::resolve_template_impl(&s.library, &id)
        .map(Json)
        .map_err(ApiError::not_found)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct StartTemplateRequest {
    pub preset_id: String,
    pub bpm: f32,
    #[serde(default)]
    pub reverse: bool,
}

#[utoipa::path(
    post,
    path = "/api/templates/start",
    request_body = StartTemplateRequest,
    responses((status = 200))
)]
async fn start_template_preset(
    State(s): State<AppState>,
    Json(body): Json<StartTemplateRequest>,
) -> ApiResult<StatusCode> {
    let mut preset = crate::commands::resolve_template_impl(&s.library, &body.preset_id)
        .map_err(ApiError::bad_request)?;
    if body.reverse {
        preset = preset.reversed();
    }
    if !body.bpm.is_finite() || body.bpm <= 0.0 {
        return Err(ApiError::bad_request(format!("invalid bpm: {}", body.bpm)));
    }
    send_audio(
        &s.audio,
        AudioCommand::StartTemplate {
            template: preset,
            bpm: body.bpm,
        },
    )?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/templates/abort", responses((status = 200)))]
async fn abort_template(State(s): State<AppState>) -> ApiResult<StatusCode> {
    send_audio(&s.audio, AudioCommand::AbortTemplate)?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct OverrideRequest {
    pub target_key: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ResumeRequest {
    pub target_key: String,
    pub duration_beats: Option<f64>,
}

#[utoipa::path(post, path = "/api/templates/override", request_body = OverrideRequest, responses((status = 200)))]
async fn override_param(
    State(s): State<AppState>,
    Json(body): Json<OverrideRequest>,
) -> ApiResult<StatusCode> {
    let target = crate::audio_engine::key_to_target(&body.target_key)
        .map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::OverrideParam { target })?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/templates/resume", request_body = ResumeRequest, responses((status = 200)))]
async fn resume_param(
    State(s): State<AppState>,
    Json(body): Json<ResumeRequest>,
) -> ApiResult<StatusCode> {
    let target = crate::audio_engine::key_to_target(&body.target_key)
        .map_err(ApiError::bad_request)?;
    let dur = body.duration_beats.unwrap_or(4.0).max(0.25);
    send_audio(
        &s.audio,
        AudioCommand::ResumeParam {
            target,
            duration_beats: dur,
        },
    )?;
    Ok(StatusCode::OK)
}

#[utoipa::path(post, path = "/api/templates/commit", request_body = OverrideRequest, responses((status = 200)))]
async fn commit_param(
    State(s): State<AppState>,
    Json(body): Json<OverrideRequest>,
) -> ApiResult<StatusCode> {
    let target = crate::audio_engine::key_to_target(&body.target_key)
        .map_err(ApiError::bad_request)?;
    send_audio(&s.audio, AudioCommand::CommitParam { target })?;
    Ok(StatusCode::OK)
}

// ---- Setlists -----------------------------------------------------------

#[utoipa::path(get, path = "/api/setlists", responses((status = 200, body = Object)))]
async fn list_setlists(
    State(s): State<AppState>,
) -> ApiResult<Json<Vec<conduction_core::Setlist>>> {
    s.setlists
        .list()
        .map(Json)
        .map_err(|e| ApiError::internal(e.to_string()))
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateSetlistRequest {
    pub name: String,
}

#[utoipa::path(
    post,
    path = "/api/setlists",
    request_body = CreateSetlistRequest,
    responses((status = 200, body = Object))
)]
async fn create_setlist(
    State(s): State<AppState>,
    Json(body): Json<CreateSetlistRequest>,
) -> ApiResult<Json<conduction_core::Setlist>> {
    s.setlists
        .create(body.name)
        .map(Json)
        .map_err(|e| ApiError::internal(e.to_string()))
}

#[utoipa::path(delete, path = "/api/setlists/{id}", responses((status = 200)))]
async fn delete_setlist(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let uuid = Uuid::parse_str(&id)
        .map_err(|e| ApiError::bad_request(format!("invalid setlist id: {e}")))?;
    s.setlists
        .delete(conduction_core::SetlistId::from_uuid(uuid))
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct AddSetlistEntryRequest {
    pub track_id: String,
}

#[utoipa::path(
    post,
    path = "/api/setlists/{id}/entries",
    request_body = AddSetlistEntryRequest,
    responses((status = 200, body = Object))
)]
async fn setlist_add_entry(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AddSetlistEntryRequest>,
) -> ApiResult<Json<conduction_core::SetlistEntry>> {
    let sid_uuid = Uuid::parse_str(&id)
        .map_err(|e| ApiError::bad_request(format!("invalid setlist id: {e}")))?;
    let tid_uuid = Uuid::parse_str(&body.track_id)
        .map_err(|e| ApiError::bad_request(format!("invalid track id: {e}")))?;
    let entry = s
        .setlists
        .add_entry(
            conduction_core::SetlistId::from_uuid(sid_uuid),
            conduction_core::TrackId::from_uuid(tid_uuid),
        )
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    Ok(Json(entry))
}

#[utoipa::path(
    delete,
    path = "/api/setlists/{id}/entries/{entry_id}",
    responses((status = 200))
)]
async fn setlist_remove_entry(
    State(s): State<AppState>,
    Path((id, entry_id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    let sid_uuid = Uuid::parse_str(&id)
        .map_err(|e| ApiError::bad_request(format!("invalid setlist id: {e}")))?;
    let eid_uuid = Uuid::parse_str(&entry_id)
        .map_err(|e| ApiError::bad_request(format!("invalid entry id: {e}")))?;
    s.setlists
        .remove_entry(
            conduction_core::SetlistId::from_uuid(sid_uuid),
            conduction_core::SetlistEntryId::from_uuid(eid_uuid),
        )
        .map_err(|e| ApiError::not_found(e.to_string()))?;
    Ok(StatusCode::OK)
}

// ---- Cue dynamic matching ----------------------------------------------

#[utoipa::path(
    post,
    path = "/api/match",
    request_body = MatchQueryArgs,
    responses((status = 200, body = Vec<MatchCandidateDto>))
)]
async fn list_match_candidates(
    State(s): State<AppState>,
    Json(args): Json<MatchQueryArgs>,
) -> ApiResult<Json<Vec<MatchCandidateDto>>> {
    crate::commands::list_match_candidates_impl(&s.library, args)
        .map(Json)
        .map_err(ApiError::internal)
}

// ============================================================================
// Helpers
// ============================================================================

fn analyze_and_save(
    library: &Arc<Mutex<Library>>,
    track_id: TrackId,
    path: &std::path::Path,
) -> Result<WaveformPreview, String> {
    let audio = decode_to_pcm(path).map_err(|e| e.to_string())?;
    let total_sec = audio.duration_sec();
    let wf = generate_waveform(&audio, DEFAULT_WAVEFORM_BINS);
    let estimate = estimate_beatgrid(&audio);
    let key_estimate = estimate_key(&audio);
    let mut lib = library.lock();
    lib.save_waveform(track_id, &wf).map_err(|e| e.to_string())?;
    if let Some(est) = estimate {
        let beats = est.beats(total_sec);
        lib.save_track_analysis(track_id, est.bpm, &beats)
            .map_err(|e| e.to_string())?;
    }
    if let Some(k) = key_estimate {
        lib.save_track_key(track_id, k.key)
            .map_err(|e| e.to_string())?;
    }
    Ok(wf)
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub ok: bool,
}

// ============================================================================
// OpenAPI document
// ============================================================================

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Conduction Local API",
        description = "Programmable DJ engine — full parameter access over localhost HTTP. Bound to 127.0.0.1, no auth.",
        version = "0.1.0"
    ),
    paths(
        health,
        get_status,
        list_audio_devices,
        get_resources,
        get_settings,
        put_settings,
        list_tracks,
        import_track,
        delete_track,
        analyze_track,
        get_waveform,
        get_beats,
        list_hot_cues,
        set_hot_cue,
        delete_hot_cue,
        load_track,
        play,
        pause,
        stop,
        seek_deck,
        loop_in,
        loop_out,
        loop_toggle,
        loop_clear,
        set_eq,
        set_filter,
        set_echo,
        set_reverb,
        set_cue_send,
        set_key_lock,
        set_pitch_offset,
        set_channel_volume,
        set_tempo_adjust,
        set_tempo_range,
        set_crossfader,
        set_master_volume,
        yt_available,
        yt_search,
        yt_download,
        export_preview,
        export_execute,
        list_cues_for_track,
        insert_cue,
        delete_cue,
        list_match_candidates,
        list_template_presets,
        get_template_preset,
        start_template_preset,
        abort_template,
        override_param,
        resume_param,
        commit_param,
        list_setlists,
        create_setlist,
        delete_setlist,
        setlist_add_entry,
        setlist_remove_entry,
    ),
    components(schemas(
        HealthResponse,
        MixerSnapshot,
        crate::audio_engine::DeckSnapshot,
        ResourceStats,
        AppSettings,
        KeybindingEntry,
        TrackSummary,
        BeatDto,
        HotCueDto,
        ImportTrackRequest,
        SetHotCueRequest,
        LoadTrackRequest,
        SeekRequest,
        PositionRequest,
        EqRequest,
        ScalarRequest,
        EchoRequest,
        ReverbRequest,
        VolumeRequest,
        TempoAdjustRequest,
        TempoRangeRequest,
        CrossfaderRequest,
        KeyLockRequest,
        PitchOffsetRequest,
        VideoSearchResult,
        AudioFormat,
        YtAvailableResponse,
        YtDownloadRequest,
        ExportRequest,
        ExportPreview,
        ExportReport,
        CueDto,
        InsertCueArgs,
        MatchCandidateDto,
        MatchQueryArgs,
        TemplatePresetDto,
        StartTemplateRequest,
        crate::audio_engine::TemplateStatus,
        crate::audio_engine::AutomationModeEntry,
        OverrideRequest,
        ResumeRequest,
        CreateSetlistRequest,
        AddSetlistEntryRequest,
    )),
    tags(
        (name = "status", description = "Mixer/deck snapshot"),
        (name = "deck", description = "Deck transport / DSP / loop / EQ"),
        (name = "mixer", description = "Crossfader and master volume"),
        (name = "library", description = "Track import / waveform / hot cues"),
        (name = "system", description = "Audio devices / resources / settings"),
        (name = "youtube", description = "yt-dlp search and download"),
        (name = "export", description = "rekordbox-compatible USB export (Phase 1: skeleton)"),
        (name = "cue", description = "Typed cues (intro/breakdown/drop/outro/...) for dynamic matching"),
        (name = "match", description = "Dynamic cue matching from active deck state"),
        (name = "template", description = "Transition templates: list presets / start / abort"),
        (name = "setlist", description = "Setlists (Phase A1: in-memory only)"),
    )
)]
struct ApiDoc;
