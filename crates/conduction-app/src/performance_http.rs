//! Local HTTP equivalents of the desktop performance endpoints.
use crate::{http_api::AppState, performance::PerformanceBrowser};
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use conduction_audio::AudioOutputConfig;
use conduction_link::LinkConfig;
use conduction_midi::{ControllerAction, MidiConfig, MidiService};
use serde::Deserialize;

type ApiResult = Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)>;
fn error(error: impl std::fmt::Display) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error":error.to_string()})),
    )
}
fn json(value: impl serde::Serialize) -> ApiResult {
    serde_json::to_value(value).map(Json).map_err(error)
}
fn done(result: anyhow::Result<()>) -> ApiResult {
    result.map_err(error)?;
    json(serde_json::json!({"ok":true}))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/performance", get(status))
        .route("/api/performance/actions", post(perform))
        .route("/api/performance/browser", post(browser))
        .route("/api/audio/outputs", get(audio_outputs))
        .route("/api/audio/configure", post(configure_audio))
        .route("/api/link/interfaces", get(interfaces))
        .route("/api/link/configure", post(configure_link))
        .route("/api/link/library/refresh", post(refresh_library))
        .route("/api/link/master", post(master))
        .route("/api/midi/devices", get(midi_devices))
        .route("/api/midi/profiles", get(midi_profiles))
        .route("/api/midi/configure", post(configure_midi))
        .route("/api/midi/disconnect", post(disconnect_midi))
}

#[utoipa::path(get,path="/api/performance",responses((status=200,body=Object)),tag="Performance")]
async fn status(State(s): State<AppState>) -> ApiResult {
    json(s.performance.status())
}
#[utoipa::path(post,path="/api/performance/actions",request_body=Object,responses((status=200),(status=400)),tag="Performance")]
async fn perform(State(s): State<AppState>, Json(action): Json<ControllerAction>) -> ApiResult {
    done(s.performance.perform(action).await)
}
#[utoipa::path(post,path="/api/performance/browser",request_body=Object,responses((status=200),(status=400)),tag="Performance")]
async fn browser(State(s): State<AppState>, Json(browser): Json<PerformanceBrowser>) -> ApiResult {
    done(s.performance.set_browser(browser))
}
#[utoipa::path(get,path="/api/audio/outputs",responses((status=200,body=[Object])),tag="Performance")]
async fn audio_outputs() -> ApiResult {
    json(conduction_audio::list_audio_outputs())
}
#[utoipa::path(post,path="/api/audio/configure",request_body=Object,responses((status=200),(status=400)),tag="Performance")]
async fn configure_audio(
    State(s): State<AppState>,
    Json(config): Json<AudioOutputConfig>,
) -> ApiResult {
    done(s.performance.configure_audio(config).await)
}
#[utoipa::path(get,path="/api/link/interfaces",responses((status=200,body=[Object])),tag="Pro DJ Link")]
async fn interfaces() -> ApiResult {
    json(crate::performance::list_link_interfaces().map_err(error)?)
}
#[utoipa::path(post,path="/api/link/configure",request_body=Object,responses((status=200),(status=400)),tag="Pro DJ Link")]
async fn configure_link(State(s): State<AppState>, Json(config): Json<LinkConfig>) -> ApiResult {
    done(s.performance.configure_link(config).await)
}
#[utoipa::path(post,path="/api/link/library/refresh",responses((status=200),(status=400)),tag="Pro DJ Link")]
async fn refresh_library(State(s): State<AppState>) -> ApiResult {
    done(s.performance.refresh_library().await)
}
#[utoipa::path(post,path="/api/link/master",responses((status=200),(status=400)),tag="Pro DJ Link")]
async fn master(State(s): State<AppState>) -> ApiResult {
    done(s.performance.request_master())
}
#[utoipa::path(get,path="/api/midi/devices",responses((status=200,body=Object)),tag="MIDI")]
async fn midi_devices(State(s): State<AppState>) -> ApiResult {
    json(s.performance.midi().devices().map_err(error)?)
}
#[utoipa::path(get,path="/api/midi/profiles",responses((status=200,body=[Object])),tag="MIDI")]
async fn midi_profiles() -> ApiResult {
    json(MidiService::profiles())
}
#[utoipa::path(post,path="/api/midi/configure",request_body=Object,responses((status=200),(status=400)),tag="MIDI")]
async fn configure_midi(State(s): State<AppState>, Json(config): Json<MidiConfig>) -> ApiResult {
    done(s.performance.configure_midi(config))
}
#[derive(Deserialize)]
struct Disconnect {
    input_port: Option<String>,
}
#[utoipa::path(post,path="/api/midi/disconnect",request_body=Object,responses((status=200),(status=400)),tag="MIDI")]
async fn disconnect_midi(State(s): State<AppState>, Json(config): Json<Disconnect>) -> ApiResult {
    done(s.performance.disconnect_midi(config.input_port))
}

#[derive(utoipa::OpenApi)]
#[openapi(paths(
    status,
    perform,
    browser,
    audio_outputs,
    configure_audio,
    interfaces,
    configure_link,
    refresh_library,
    master,
    midi_devices,
    midi_profiles,
    configure_midi,
    disconnect_midi
))]
pub struct PerformanceApiDoc;
