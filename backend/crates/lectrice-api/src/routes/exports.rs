use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_core::Stream;
use lectrice_app::UseCase;
use lectrice_app::exports::{ExportDownload, StartAudioExportArgs};
use lectrice_domain::{AppError, AudioExport, DocumentId, ErrorName, ExportFormat, ExportId, ExportSpeed, VoiceId};
use serde::Deserialize;
use serde_json::json;
use tokio::io::{AsyncRead, ReadBuf};

use super::extract::{ValidJson, invalid, run_blocking};
use crate::context::Ctx;
use crate::error::ApiError;
use crate::state::AppState;

const MP3_MIME_TYPE: &str = "audio/mpeg";
const M4B_MIME_TYPE: &str = "audio/mp4";
const FILE_CHUNK_BYTES: usize = 64 * 1024;
/// Keeps the download name readable and well under file-system limits.
const MAX_FILE_STEM_CHARS: usize = 150;
const FALLBACK_FILE_STEM: &str = "Lectrice export";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartExportBody {
    voice_id: String,
    format: String,
    read_skipped: Option<bool>,
}

impl StartExportBody {
    fn validate(self, document_id: DocumentId) -> Result<StartAudioExportArgs, ApiError> {
        let voice = self.voice_id.trim();
        if voice.is_empty() {
            return Err(invalid("voiceId must not be empty"));
        }
        let format = self.format.trim().to_ascii_lowercase();
        let Some(format) = ExportFormat::from_extension(&format) else {
            return Err(ApiError(AppError::new(ErrorName::ExportFormatUnsupported).with_detail(format!("format {format:?}"))));
        };
        Ok(StartAudioExportArgs {
            document_id,
            voice_id: VoiceId::new(voice),
            format,
            read_skipped: self.read_skipped.unwrap_or(false),
            speed: ExportSpeed::NORMAL,
            engine_id: None,
            range: None,
            range_label: None,
        })
    }
}

fn export_id(raw: &str) -> ExportId {
    ExportId::new(raw.trim())
}

fn present(export: AudioExport) -> Json<serde_json::Value> {
    Json(json!({ "export": export }))
}

/// `POST /api/documents/{id}/exports` with `{ voiceId, format: "mp3" | "m4b", readSkipped }` → 202.
pub async fn start_export(
    State(state): State<AppState>,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
    ValidJson(body): ValidJson<StartExportBody>,
) -> Result<Response, ApiError> {
    let args = body.validate(DocumentId::parse(&id)?)?;
    let exports = state.services.exports()?;
    let export = run_blocking(move || exports.start.run(&ctx, args)).await?;
    Ok((StatusCode::ACCEPTED, present(export)).into_response())
}

/// `GET /api/documents/{id}/exports` → `{ items }`, newest first.
pub async fn list_exports(State(state): State<AppState>, Ctx(ctx): Ctx, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let document_id = DocumentId::parse(&id)?;
    let exports = state.services.exports()?;
    let items = run_blocking(move || exports.list.run(&ctx, document_id)).await?;
    Ok(Json(json!({ "items": items })))
}

/// `GET /api/exports/{exportId}`
pub async fn get_export(State(state): State<AppState>, Ctx(ctx): Ctx, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let exports = state.services.exports()?;
    let export = run_blocking(move || exports.get.run(&ctx, export_id(&id))).await?;
    Ok(present(export))
}

/// `DELETE /api/exports/{exportId}` cancels → 204.
pub async fn cancel_export(State(state): State<AppState>, Ctx(ctx): Ctx, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    let exports = state.services.exports()?;
    run_blocking(move || exports.cancel.run(&ctx, export_id(&id))).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/exports/{exportId}/file`: the audio file, streamed, named `<Title> (<Voice>).<ext>`.
pub async fn download_export(State(state): State<AppState>, Ctx(ctx): Ctx, Path(id): Path<String>) -> Result<Response, ApiError> {
    let exports = state.services.exports()?;
    let download = run_blocking(move || exports.file.run(&ctx, export_id(&id))).await?;
    let file = tokio::fs::File::open(&download.path).await.map_err(|error| file_missing(&download, error))?;
    let length = file.metadata().await.map_err(|error| file_missing(&download, error))?.len();

    let headers = [
        (CONTENT_TYPE, HeaderValue::from_static(mime_type(download.format))),
        (CONTENT_LENGTH, HeaderValue::from(length)),
        (CONTENT_DISPOSITION, content_disposition(&download)),
    ];
    Ok((headers, Body::from_stream(FileChunks { file, buffer: vec![0; FILE_CHUNK_BYTES] })).into_response())
}

fn file_missing(download: &ExportDownload, error: io::Error) -> ApiError {
    ApiError(AppError::new(ErrorName::ExportNotFound).with_detail(format!("export file {}", download.path.display())).with_source(error))
}

const fn mime_type(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Mp3 => MP3_MIME_TYPE,
        ExportFormat::M4b => M4B_MIME_TYPE,
    }
}

/// `attachment` with a plain ASCII `filename` for old clients and the exact UTF-8 name in `filename*` (RFC 6266).
fn content_disposition(download: &ExportDownload) -> HeaderValue {
    let name = format!("{} ({}).{}", file_stem(&download.title), file_stem(&download.voice_name), download.format.extension());
    let ascii: String = name.chars().map(|character| if character.is_ascii() && character != '"' { character } else { '_' }).collect();
    let value = format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{}", percent_encode(&name));
    HeaderValue::from_str(&value).unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

/// Drops characters that file systems or headers reject and squeezes whitespace.
fn file_stem(text: &str) -> String {
    let cleaned: String = text.chars().map(|character| if character.is_control() || r#"/\:*?"<>|"#.contains(character) { ' ' } else { character }).collect();
    let stem: String = cleaned.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_FILE_STEM_CHARS).collect();
    let stem = stem.trim().trim_matches('.').to_string();
    if stem.is_empty() { FALLBACK_FILE_STEM.to_string() } else { stem }
}

/// RFC 5987 `attr-char`s stay; every other byte is `%XX`.
fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// Reads the file in chunks as the client takes them, so a 230 MB book never sits in memory.
struct FileChunks {
    file: tokio::fs::File,
    buffer: Vec<u8>,
}

impl Stream for FileChunks {
    type Item = io::Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let mut buffer = ReadBuf::new(&mut this.buffer);
        if let Err(error) = ready!(Pin::new(&mut this.file).poll_read(cx, &mut buffer)) {
            return Poll::Ready(Some(Err(error)));
        }
        let filled = buffer.filled();
        if filled.is_empty() {
            return Poll::Ready(None);
        }
        Poll::Ready(Some(Ok(Bytes::copy_from_slice(filled))))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use axum::body::Body;
    use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
    use axum::http::{Request, StatusCode};
    use serde_json::Value;

    use super::*;
    use crate::routes::DOCUMENTS_PATH;
    use crate::test_support::{TestApp, error_name, get};

    const WAIT_LIMIT: Duration = Duration::from_secs(10);

    fn exports_path(id: &str) -> String {
        format!("{DOCUMENTS_PATH}/{id}/exports")
    }

    fn post_json(path: &str, body: &str) -> Request<Body> {
        Request::builder().method("POST").uri(path).header(CONTENT_TYPE, "application/json").body(Body::from(body.to_string())).unwrap()
    }

    fn delete(path: &str) -> Request<Body> {
        Request::builder().method("DELETE").uri(path).body(Body::empty()).unwrap()
    }

    async fn start(app: &TestApp, id: &str, format: &str) -> Value {
        let body = format!(r#"{{ "voiceId": "af_heart", "format": "{format}", "readSkipped": false }}"#);
        let (status, body) = app.json(post_json(&exports_path(id), &body)).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{body}");
        body["export"].clone()
    }

    async fn wait_until_finished(app: &TestApp, export_id: &str) -> Value {
        let started = Instant::now();
        loop {
            let (status, body) = app.json(get(&format!("/api/exports/{export_id}"))).await;
            assert_eq!(status, StatusCode::OK);
            let export = body["export"].clone();
            if !matches!(export["status"].as_str(), Some("queued" | "running")) {
                return export;
            }
            assert!(started.elapsed() < WAIT_LIMIT, "export never finished: {export}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn an_export_runs_to_done_and_downloads_with_a_readable_name() {
        let app = TestApp::new();
        let id = app.import_book().await;

        let queued = start(&app, &id, "mp3").await;
        assert_eq!(queued["status"], "queued");
        assert_eq!((queued["documentId"].as_str(), queued["voiceId"].as_str(), queued["format"].as_str()), (Some(id.as_str()), Some("af_heart"), Some("mp3")));
        assert_eq!((queued["sentencesDone"].as_u64(), queued["sentencesTotal"].as_u64()), (Some(0), Some(5)));
        assert!(queued["estimatedSecondsLeft"].is_null());
        let export_id = queued["id"].as_str().unwrap().to_string();

        let done = wait_until_finished(&app, &export_id).await;
        assert_eq!((done["status"].as_str(), done["sentencesDone"].as_u64(), done["estimatedSecondsLeft"].as_u64()), (Some("done"), Some(5), Some(0)));

        let (status, headers, bytes) = app.send(get(&format!("/api/exports/{export_id}/file"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[CONTENT_TYPE], "audio/mpeg");
        assert_eq!(headers[CONTENT_DISPOSITION], r#"attachment; filename="Fake Book (Heart).mp3"; filename*=UTF-8''Fake%20Book%20%28Heart%29.mp3"#);
        assert_eq!(bytes.as_ref(), app.exporter.file_bytes().as_slice());

        let (status, body) = app.json(get(&exports_path(&id))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["items"][0]["id"], export_id.as_str());
    }

    #[tokio::test]
    async fn exports_are_listed_newest_first_and_cancel_is_204() {
        let app = TestApp::with_blocked_exports();
        let id = app.import_book().await;
        let mp3 = start(&app, &id, "mp3").await;
        let m4b = start(&app, &id, "M4B").await;
        assert_eq!(m4b["format"], "m4b");

        let (_, body) = app.json(get(&exports_path(&id))).await;
        let ids: Vec<&str> = body["items"].as_array().unwrap().iter().map(|item| item["id"].as_str().unwrap()).collect();
        assert_eq!(ids, [m4b["id"].as_str().unwrap(), mp3["id"].as_str().unwrap()]);

        let m4b_path = format!("/api/exports/{}", m4b["id"].as_str().unwrap());
        let (status, _, bytes) = app.send(delete(&m4b_path)).await;
        assert_eq!((status, bytes.len()), (StatusCode::NO_CONTENT, 0));
        let (_, body) = app.json(get(&m4b_path)).await;
        assert_eq!(body["export"]["status"], "cancelled");
        let (status, body) = app.json(get(&format!("{m4b_path}/file"))).await;
        assert_eq!((status, error_name(&body)), (StatusCode::CONFLICT, "ExportNotReady"));
    }

    #[tokio::test]
    async fn bad_requests_get_registry_errors() {
        let app = TestApp::new();
        let id = app.import_book().await;
        let unknown_document = "00000000000000000000000000000000000000000000000000000000000000ff";
        let cases = [
            (post_json(&exports_path(&id), r#"{ "voiceId": "af_heart", "format": "wav" }"#), StatusCode::BAD_REQUEST, "ExportFormatUnsupported"),
            (post_json(&exports_path(&id), r#"{ "voiceId": "zz_nobody", "format": "mp3" }"#), StatusCode::BAD_REQUEST, "VoiceUnknown"),
            (post_json(&exports_path(&id), r#"{ "voiceId": " ", "format": "mp3" }"#), StatusCode::BAD_REQUEST, "RequestInvalid"),
            (post_json(&exports_path(&id), r#"{ "format": "mp3" }"#), StatusCode::BAD_REQUEST, "RequestInvalid"),
            (post_json(&exports_path(&id), r#"{ "voiceId": "af_heart", "format": "mp3", "speed": 2 }"#), StatusCode::BAD_REQUEST, "RequestInvalid"),
            (post_json(&exports_path(&id), "not json"), StatusCode::BAD_REQUEST, "RequestInvalid"),
            (post_json(&exports_path(unknown_document), r#"{ "voiceId": "af_heart", "format": "mp3" }"#), StatusCode::NOT_FOUND, "DocumentNotFound"),
            (post_json(&exports_path("nope"), r#"{ "voiceId": "af_heart", "format": "mp3" }"#), StatusCode::NOT_FOUND, "DocumentNotFound"),
            (get(&exports_path(unknown_document)), StatusCode::NOT_FOUND, "DocumentNotFound"),
            (get("/api/exports/nope"), StatusCode::NOT_FOUND, "ExportNotFound"),
            (get("/api/exports/nope/file"), StatusCode::NOT_FOUND, "ExportNotFound"),
            (delete("/api/exports/nope"), StatusCode::NOT_FOUND, "ExportNotFound"),
        ];

        for (request, expected_status, expected_name) in cases {
            let path = request.uri().to_string();
            let (status, body) = app.json(request).await;
            assert_eq!((status, error_name(&body)), (expected_status, expected_name), "{path}");
        }
    }

    #[tokio::test]
    async fn a_missing_exporter_is_503() {
        let app = TestApp::without_exporter();
        let id = app.import_book().await;

        let (status, body) = app.json(post_json(&exports_path(&id), r#"{ "voiceId": "af_heart", "format": "m4b", "readSkipped": true }"#)).await;

        assert_eq!((status, error_name(&body)), (StatusCode::SERVICE_UNAVAILABLE, "ExporterUnavailable"));
    }

    #[tokio::test]
    async fn without_the_speech_engine_exports_are_unavailable() {
        let app = TestApp::without_engine();
        let id = app.import_book().await;

        let (status, body) = app.json(get(&exports_path(&id))).await;

        assert_eq!((status, error_name(&body)), (StatusCode::SERVICE_UNAVAILABLE, "SpeechEngineUnavailable"));
    }

    #[test]
    fn download_names_are_safe_and_keep_unicode_in_filename_star() {
        let download = |title: &str| ExportDownload { path: PathBuf::new(), format: ExportFormat::M4b, title: title.into(), voice_name: "Heart".into() };

        let header = content_disposition(&download("Café: \"A/B\"\nnotes"));
        assert_eq!(header, r#"attachment; filename="Caf_ A B notes (Heart).m4b"; filename*=UTF-8''Caf%C3%A9%20A%20B%20notes%20%28Heart%29.m4b"#);
        assert!(content_disposition(&download(" ... ")).to_str().unwrap().contains("filename=\"Lectrice export (Heart).m4b\""));
    }
}
