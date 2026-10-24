use axum::Json;
use axum::extract::{Path, State};
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use lectrice_app::UseCase;
use lectrice_app::speech::VoiceList;
use lectrice_domain::{Voice, VoiceId};
use serde::Serialize;

use super::extract::{CancelOnDrop, invalid, run_blocking};
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
    let speech = state.services.speech()?;
    let list = run_blocking(move || speech.list_voices.run(&ctx, ())).await?;
    Ok(present(list))
}

/// `GET /api/voices/{voiceId}/sample`: the voice reading one fixed sentence (made once, then kept).
pub async fn get_voice_sample(State(state): State<AppState>, Ctx(ctx): Ctx, Path(voice): Path<String>) -> Result<Response, ApiError> {
    let voice = voice.trim();
    if voice.is_empty() {
        return Err(invalid("voiceId must not be empty"));
    }
    let voice_id = VoiceId::new(voice);
    let speech = state.services.speech()?;
    let _cancel_on_disconnect = CancelOnDrop(ctx.cancellation.clone());
    let sample = run_blocking(move || speech.voice_sample.run(&ctx, voice_id)).await?;
    Ok(([(CONTENT_TYPE, sample.mime_type)], sample.bytes).into_response())
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
    async fn a_voice_sample_is_made_once_and_then_read_from_disk() {
        let app = TestApp::new();

        let (status, headers, first) = app.send(get("/api/voices/af_heart/sample")).await;
        let (_, _, second) = app.send(get("/api/voices/af_heart/sample")).await;

        assert_eq!((status, headers[axum::http::header::CONTENT_TYPE].to_str().unwrap()), (StatusCode::OK, "audio/wav"));
        assert_eq!(first, second);
        assert_eq!(app.engine.calls(), 1);
        assert!(app.data_dir.path().join("samples/fake-engine/af_heart.v1.wav").is_file());
        let (status, body) = app.json(get("/api/voices/zz_nobody/sample")).await;
        assert_eq!((status, error_name(&body)), (StatusCode::BAD_REQUEST, "VoiceUnknown"));
    }

    #[tokio::test]
    async fn a_failed_engine_is_speech_engine_unavailable() {
        let (status, body) = TestApp::without_engine().json(get(VOICES_PATH)).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error_name(&body), "SpeechEngineUnavailable");
    }
}
