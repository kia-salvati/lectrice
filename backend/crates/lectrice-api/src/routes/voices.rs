use axum::Json;
use axum::extract::State;
use lectrice_app::UseCase;
use lectrice_app::speech::VoiceList;
use lectrice_domain::{Voice, VoiceId};
use serde::Serialize;

use super::extract::run_blocking;
use crate::context::Ctx;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceListResponse {
    voices: Vec<Voice>,
    default_voice_id: VoiceId,
}

fn present(list: VoiceList) -> Json<VoiceListResponse> {
    Json(VoiceListResponse { voices: list.voices, default_voice_id: list.default_voice_id })
}

/// `GET /api/voices`
pub async fn list_voices(State(state): State<AppState>, Ctx(ctx): Ctx) -> Result<Json<VoiceListResponse>, ApiError> {
    let speech = state.speech()?;
    let list = run_blocking(move || speech.list_voices.run(&ctx, ())).await?;
    Ok(present(list))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use crate::routes::VOICES_PATH;
    use crate::test_support::{TestApp, error_name, get};

    #[tokio::test]
    async fn lists_the_engine_voices_and_the_default() {
        let (status, body) = TestApp::new().json(get(VOICES_PATH)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "voices": [{ "id": "af_heart", "name": "Heart", "language": "en-US", "gender": "female" }], "defaultVoiceId": "af_heart" }));
    }

    #[tokio::test]
    async fn a_failed_engine_is_speech_engine_unavailable() {
        let (status, body) = TestApp::without_engine().json(get(VOICES_PATH)).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error_name(&body), "SpeechEngineUnavailable");
    }
}
