use axum::extract::{Path, State};
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use lectrice_app::UseCase;
use lectrice_app::speech::{SpeakSentenceArgs, SpokenSentence};
use lectrice_domain::{DocumentId, VoiceId};
use serde::Deserialize;

use super::extract::{CancelOnDrop, ValidQuery, invalid, run_blocking, sentence_id};
use crate::context::Ctx;
use crate::error::ApiError;
use crate::state::AppState;

/// The player's speed range (spec §4).
const MIN_RATE: f32 = 0.75;
const MAX_RATE: f32 = 2.0;
const DEFAULT_RATE: f32 = 1.0;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioQuery {
    voice: String,
    rate: Option<f32>,
    read_skipped: Option<bool>,
}

struct ValidAudioQuery {
    voice_id: VoiceId,
    rate: f32,
    read_skipped: bool,
}

impl AudioQuery {
    fn validate(self) -> Result<ValidAudioQuery, ApiError> {
        let voice = self.voice.trim();
        if voice.is_empty() {
            return Err(invalid("voice must not be empty"));
        }
        let rate = self.rate.unwrap_or(DEFAULT_RATE);
        if !(MIN_RATE..=MAX_RATE).contains(&rate) {
            return Err(invalid(format!("rate must be between {MIN_RATE} and {MAX_RATE}")));
        }
        Ok(ValidAudioQuery { voice_id: VoiceId::new(voice), rate, read_skipped: self.read_skipped.unwrap_or(false) })
    }
}

fn present(spoken: SpokenSentence) -> Response {
    ([(CONTENT_TYPE, spoken.mime_type)], spoken.bytes).into_response()
}

/// `GET /api/documents/{id}/sentences/{sentenceId}/audio?voice=af_heart&rate=1.5&readSkipped=false`
pub async fn get_sentence_audio(
    State(state): State<AppState>,
    Ctx(ctx): Ctx,
    Path((id, sentence)): Path<(String, String)>,
    ValidQuery(query): ValidQuery<AudioQuery>,
) -> Result<Response, ApiError> {
    let query = query.validate()?;
    let args = SpeakSentenceArgs {
        document_id: DocumentId::parse(&id)?,
        sentence_id: sentence_id(&sentence)?,
        voice_id: query.voice_id,
        rate: query.rate,
        read_skipped: query.read_skipped,
    };
    let speech = state.speech()?;
    let _cancel_on_disconnect = CancelOnDrop(ctx.cancellation.clone());
    let spoken = run_blocking(move || speech.speak_sentence.run(&ctx, args)).await?;
    Ok(present(spoken))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use axum::http::StatusCode;
    use axum::http::header::CONTENT_TYPE;

    use crate::composition::build_speech_engine;
    use crate::config::{SpeechConfig, SpeechEngineKind};
    use crate::routes::DOCUMENTS_PATH;
    use crate::test_support::{SPEC_READ_AHEAD, TestApp, error_name, get, sentences};

    const WAV_HEADER_BYTES: usize = 44;
    const KOKORO_MODEL_DIR: &str = "../../../models/kokoro-multi-lang-v1_0";

    fn audio_path(id: &str, sentence: impl std::fmt::Display, query: &str) -> String {
        format!("{DOCUMENTS_PATH}/{id}/sentences/{sentence}/audio?{query}")
    }

    fn is_wav(bytes: &[u8]) -> bool {
        bytes.len() > WAV_HEADER_BYTES && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE"
    }

    #[tokio::test]
    async fn returns_wav_and_caches_it() {
        let app = TestApp::new();
        let id = app.import_book().await;
        let path = audio_path(&id, sentences::FIRST, "voice=af_heart&rate=1.5&readSkipped=false");

        let (status, headers, first) = app.send(get(&path)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[CONTENT_TYPE], "audio/wav");
        assert!(is_wav(&first));

        let (_, _, second) = app.send(get(&path)).await;
        assert_eq!(first, second);
        assert_eq!(app.engine.calls(), 1);
        let cached = app.data_dir.path().join("audio").join(&id).join("c1/fake-engine/af_heart/1.wav");
        assert!(cached.is_file());
    }

    #[tokio::test]
    async fn a_request_fills_the_read_ahead_in_playback_order() {
        let app = TestApp::with_read_ahead(SPEC_READ_AHEAD);
        let id = app.import_book().await;

        let (status, _, _) = app.send(get(&audio_path(&id, sentences::FIRST, "voice=af_heart"))).await;

        assert_eq!(status, StatusCode::OK);
        let voice_dir = app.data_dir.path().join("audio").join(&id).join("c1/fake-engine/af_heart");
        let started = std::time::Instant::now();
        // The URL-only sentence is silent and the code text is skipped; the code cue is read ahead.
        let expected = ["1.wav", "2.wav", "5.wav"];
        while !expected.iter().all(|name| voice_dir.join(name).is_file()) {
            assert!(started.elapsed() < std::time::Duration::from_secs(5), "read-ahead never filled the cache");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(app.engine.calls(), 3);
        assert!(!voice_dir.join("3.wav").exists() && !voice_dir.join("4.wav").exists());
    }

    #[tokio::test]
    async fn rate_and_read_skipped_are_optional_and_cues_are_spoken() {
        let app = TestApp::new();
        let id = app.import_book().await;

        let (status, _, bytes) = app.send(get(&audio_path(&id, sentences::CODE_CUE, "voice=af_heart"))).await;

        assert_eq!(status, StatusCode::OK);
        assert!(is_wav(&bytes));
        assert_eq!(app.engine.calls(), 1);
    }

    #[tokio::test]
    async fn empty_speech_text_is_short_silence_without_the_engine() {
        let app = TestApp::new();
        let id = app.import_book().await;

        let (status, _, bytes) = app.send(get(&audio_path(&id, sentences::ONLY_URL, "voice=af_heart"))).await;

        assert_eq!(status, StatusCode::OK);
        assert!(is_wav(&bytes));
        assert!(bytes[WAV_HEADER_BYTES..].iter().all(|byte| *byte == 0));
        assert_eq!(app.engine.calls(), 0);
    }

    #[tokio::test]
    async fn unknown_voice_sentence_and_document() {
        let app = TestApp::new();
        let id = app.import_book().await;
        let unknown_document = "00000000000000000000000000000000000000000000000000000000000000ff";
        let cases = [
            (audio_path(&id, sentences::FIRST, "voice=zz_nobody"), StatusCode::BAD_REQUEST, "VoiceUnknown"),
            (audio_path(&id, sentences::COUNT, "voice=af_heart"), StatusCode::NOT_FOUND, "SentenceNotFound"),
            (audio_path(&id, "abc", "voice=af_heart"), StatusCode::NOT_FOUND, "SentenceNotFound"),
            (audio_path(unknown_document, sentences::FIRST, "voice=af_heart"), StatusCode::NOT_FOUND, "DocumentNotFound"),
            (audio_path("nope", sentences::FIRST, "voice=af_heart"), StatusCode::NOT_FOUND, "DocumentNotFound"),
        ];

        for (path, expected_status, expected_name) in cases {
            let (status, body) = app.json(get(&path)).await;
            assert_eq!((status, error_name(&body)), (expected_status, expected_name), "{path}");
        }
        assert_eq!(app.engine.calls(), 0);
    }

    #[tokio::test]
    async fn bad_query_params_are_request_invalid() {
        let app = TestApp::new();
        let id = app.import_book().await;
        let queries = [
            "",
            "voice=",
            "voice=af_heart&rate=0.5",
            "voice=af_heart&rate=2.5",
            "voice=af_heart&rate=NaN",
            "voice=af_heart&rate=fast",
            "voice=af_heart&readSkipped=maybe",
            "voice=af_heart&speed=1",
        ];

        for query in queries {
            let (status, body) = app.json(get(&audio_path(&id, sentences::FIRST, query))).await;
            assert_eq!((status, error_name(&body)), (StatusCode::BAD_REQUEST, "RequestInvalid"), "{query}");
        }
    }

    #[tokio::test]
    async fn a_failed_engine_is_speech_engine_unavailable() {
        let app = TestApp::without_engine();
        let id = app.import_book().await;

        let (status, body) = app.json(get(&audio_path(&id, sentences::HEADING, "voice=af_heart"))).await;

        assert_eq!((status, error_name(&body)), (StatusCode::SERVICE_UNAVAILABLE, "SpeechEngineUnavailable"));
    }

    /// Needs the Kokoro bundle in `models/`: `cargo test -p lectrice-api -- --ignored`.
    #[tokio::test]
    #[ignore = "loads the real Kokoro model"]
    async fn the_real_engine_speaks_a_sentence() {
        let model_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(KOKORO_MODEL_DIR);
        let config = SpeechConfig { engine: SpeechEngineKind::Kokoro, model_dir, default_voice: "af_heart".into(), num_threads: 4 };
        let engine = build_speech_engine(&config).unwrap();
        let app = TestApp::with_engine(engine);
        let id = app.import_book().await;

        let (status, headers, bytes) = app.send(get(&audio_path(&id, sentences::FIRST, "voice=af_heart"))).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[CONTENT_TYPE], "audio/wav");
        assert!(is_wav(&bytes));
        assert!(bytes.len() > WAV_HEADER_BYTES + 24_000, "expected well over half a second of 16-bit audio");
    }
}
