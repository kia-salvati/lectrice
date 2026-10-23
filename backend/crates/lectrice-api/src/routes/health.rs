use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    status: HealthStatus,
    speech_engine: SpeechEngineState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum HealthStatus {
    Ok,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum SpeechEngineState {
    Ready,
    Failed,
}

pub async fn get_health(State(state): State<AppState>) -> Json<HealthResponse> {
    let speech_engine = if state.services.speech.is_some() { SpeechEngineState::Ready } else { SpeechEngineState::Failed };
    Json(HealthResponse { status: HealthStatus::Ok, speech_engine })
}
