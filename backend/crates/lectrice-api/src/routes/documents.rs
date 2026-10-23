use axum::Json;
use axum::extract::multipart::{Field, MultipartError, MultipartRejection};
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use lectrice_app::UseCase;
use lectrice_app::documents::{GetDocumentArgs, ImportPdfArgs, ImportedPdf, OpenedDocument, SaveReadingPositionArgs};
use lectrice_domain::{AppError, Document, DocumentId, DocumentSummary, ErrorName, ListOptions, Page, SentenceId};
use serde::{Deserialize, Serialize};

use super::extract::{ValidJson, ValidQuery, invalid, run_blocking};
use crate::context::Ctx;
use crate::error::ApiError;
use crate::state::AppState;

const FILE_FIELD: &str = "file";

// ---- input DTOs ----

/// The `file` part of a multipart upload, read up to the size limit.
struct UploadDto {
    file_name: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

impl ListQuery {
    fn validate(self) -> Result<ListOptions, ApiError> {
        let options = ListOptions { limit: self.limit, offset: self.offset };
        if !options.has_valid_limit() {
            return Err(invalid(format!("limit must be between 1 and {}", ListOptions::MAX_LIMIT)));
        }
        Ok(options)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PositionBody {
    sentence_id: u32,
}

// ---- presenters ----

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResponse {
    document: DocumentSummary,
    already_in_library: bool,
}

/// 201 for a new document, 200 when it was already in the library.
fn present_import(imported: ImportedPdf) -> (StatusCode, Json<ImportResponse>) {
    let status = if imported.already_in_library { StatusCode::OK } else { StatusCode::CREATED };
    (status, Json(ImportResponse { document: imported.summary, already_in_library: imported.already_in_library }))
}

/// The §5 document plus `lastSentenceId` (null when never read).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedDocumentResponse {
    #[serde(flatten)]
    document: Document,
    last_sentence_id: Option<SentenceId>,
}

fn present_opened(opened: OpenedDocument) -> Json<OpenedDocumentResponse> {
    Json(OpenedDocumentResponse { document: opened.document, last_sentence_id: opened.last_sentence_id })
}

// ---- controllers ----

/// `POST /api/documents` (multipart, field `file`)
pub async fn import_pdf(
    State(state): State<AppState>,
    Ctx(ctx): Ctx,
    multipart: Result<Multipart, MultipartRejection>,
) -> Result<(StatusCode, Json<ImportResponse>), ApiError> {
    let multipart = multipart.map_err(|rejection| AppError::new(ErrorName::UploadMissingFile).with_detail(rejection.body_text()))?;
    let upload = read_upload(multipart, state.upload_max_bytes).await?;
    let documents = state.services.documents.clone();
    let args = ImportPdfArgs { file_name: upload.file_name, bytes: upload.bytes };
    let imported = run_blocking(move || documents.import_pdf.run(&ctx, args)).await?;
    Ok(present_import(imported))
}

/// `GET /api/documents?limit=20&offset=0`
pub async fn list_documents(
    State(state): State<AppState>,
    Ctx(ctx): Ctx,
    ValidQuery(query): ValidQuery<ListQuery>,
) -> Result<Json<Page<DocumentSummary>>, ApiError> {
    let options = query.validate()?;
    let documents = state.services.documents.clone();
    let page = run_blocking(move || documents.list_recent.run(&ctx, options)).await?;
    Ok(Json(page))
}

/// `GET /api/documents/{id}`
pub async fn get_document(State(state): State<AppState>, Ctx(ctx): Ctx, Path(id): Path<String>) -> Result<Json<OpenedDocumentResponse>, ApiError> {
    let id = DocumentId::parse(&id)?;
    let documents = state.services.documents.clone();
    let opened = run_blocking(move || documents.get_document.run(&ctx, GetDocumentArgs { id })).await?;
    Ok(present_opened(opened))
}

/// `PUT /api/documents/{id}/position` with `{ sentenceId }`
pub async fn save_position(
    State(state): State<AppState>,
    Ctx(ctx): Ctx,
    Path(id): Path<String>,
    ValidJson(body): ValidJson<PositionBody>,
) -> Result<StatusCode, ApiError> {
    let args = SaveReadingPositionArgs { id: DocumentId::parse(&id)?, sentence_id: SentenceId(body.sentence_id) };
    let documents = state.services.documents.clone();
    run_blocking(move || documents.save_position.run(&ctx, args)).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- upload reading ----

/// Every part counts toward `max_bytes`, so the request is cut off as soon as it is too large.
async fn read_upload(mut multipart: Multipart, max_bytes: usize) -> Result<UploadDto, ApiError> {
    let mut total = 0;
    let mut upload = None;
    while let Some(mut field) = multipart.next_field().await.map_err(multipart_error)? {
        let is_file = upload.is_none() && field.name() == Some(FILE_FIELD);
        let file_name = field.file_name().unwrap_or_default().to_string();
        let bytes = read_field(&mut field, &mut total, max_bytes).await?;
        if is_file {
            upload = Some(UploadDto { file_name, bytes });
        }
    }
    upload.ok_or_else(|| ApiError(AppError::new(ErrorName::UploadMissingFile).with_detail("no `file` part in the upload")))
}

async fn read_field(field: &mut Field<'_>, total: &mut usize, max_bytes: usize) -> Result<Vec<u8>, ApiError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
        *total += chunk.len();
        if *total > max_bytes {
            return Err(ApiError(AppError::new(ErrorName::UploadTooLarge).with_detail(format!("upload exceeds {max_bytes} bytes"))));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn multipart_error(error: MultipartError) -> ApiError {
    let name = if error.status() == StatusCode::PAYLOAD_TOO_LARGE { ErrorName::UploadTooLarge } else { ErrorName::UploadMissingFile };
    ApiError(AppError::new(name).with_detail(error.body_text()))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::Value;

    use crate::routes::DOCUMENTS_PATH;
    use crate::test_support::{NO_TEXT_PDF_BYTES, PDF_BYTES, TestApp, UPLOAD_MAX_BYTES, error_name, get, multipart, put_json, sentences, upload};

    const UNKNOWN_ID: &str = "00000000000000000000000000000000000000000000000000000000000000ff";

    fn document_path(id: &str) -> String {
        format!("{DOCUMENTS_PATH}/{id}")
    }

    fn position_path(id: &str) -> String {
        format!("{DOCUMENTS_PATH}/{id}/position")
    }

    fn assert_error(status: StatusCode, body: &Value, expected_status: StatusCode, expected_name: &str) {
        assert_eq!((status, error_name(body)), (expected_status, expected_name), "{body}");
        assert!(body["error"]["code"].is_number() && body["error"]["message"].is_string());
    }

    #[tokio::test]
    async fn a_new_pdf_is_created_and_the_same_pdf_again_is_already_in_library() {
        let app = TestApp::new();

        let (status, first) = app.json(upload("/home/me/Book.pdf", PDF_BYTES)).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(first["alreadyInLibrary"], false);
        let summary = &first["document"];
        assert_eq!((summary["title"].as_str(), summary["sourceFileName"].as_str()), (Some("Fake Book"), Some("Book.pdf")));
        assert_eq!((summary["pageCount"].as_u64(), summary["sentenceCount"].as_u64()), (Some(2), Some(u64::from(sentences::COUNT))));
        assert!(summary["lastSentenceId"].is_null() && summary["addedAt"].is_string() && summary["lastOpenedAt"].is_string());

        let (status, second) = app.json(upload("renamed.pdf", PDF_BYTES)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(second["alreadyInLibrary"], true);
        assert_eq!(second["document"]["id"], summary["id"]);
    }

    #[tokio::test]
    async fn upload_errors_have_the_registry_shape() {
        let app = TestApp::new();
        let too_large = vec![b'%'; UPLOAD_MAX_BYTES + 1];

        let (status, body) = app.json(upload("scan.pdf", NO_TEXT_PDF_BYTES)).await;
        assert_error(status, &body, StatusCode::UNPROCESSABLE_ENTITY, "PdfNoText");
        let (status, body) = app.json(upload("big.pdf", &too_large)).await;
        assert_error(status, &body, StatusCode::PAYLOAD_TOO_LARGE, "UploadTooLarge");
        let (status, body) = app.json(upload("notes.txt", b"plain text")).await;
        assert_error(status, &body, StatusCode::UNPROCESSABLE_ENTITY, "PdfInvalid");
        let (status, body) = app.json(multipart(&[("other", None, b"x")])).await;
        assert_error(status, &body, StatusCode::BAD_REQUEST, "UploadMissingFile");
    }

    #[tokio::test]
    async fn every_part_counts_toward_the_upload_limit() {
        let app = TestApp::new();
        let filler = vec![b'x'; UPLOAD_MAX_BYTES];

        let (status, body) = app.json(multipart(&[("note", None, &filler), ("file", Some("Book.pdf"), PDF_BYTES)])).await;

        assert_error(status, &body, StatusCode::PAYLOAD_TOO_LARGE, "UploadTooLarge");
    }

    #[tokio::test]
    async fn a_body_that_is_not_multipart_is_upload_missing_file() {
        let app = TestApp::new();
        let request = axum::http::Request::builder().method("POST").uri(DOCUMENTS_PATH).body(axum::body::Body::from("hello")).unwrap();

        let (status, body) = app.json(request).await;

        assert_error(status, &body, StatusCode::BAD_REQUEST, "UploadMissingFile");
    }

    #[tokio::test]
    async fn lists_recent_documents_with_paging() {
        let app = TestApp::new();
        let id = app.import_book().await;

        let (status, body) = app.json(get(&format!("{DOCUMENTS_PATH}?limit=20&offset=0"))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!((body["total"].as_u64(), body["limit"].as_u64(), body["offset"].as_u64()), (Some(1), Some(20), Some(0)));
        assert_eq!(body["items"][0]["id"], id.as_str());

        let (status, body) = app.json(get(&format!("{DOCUMENTS_PATH}?offset=1"))).await;
        assert_eq!((status, body["items"].as_array().map(Vec::len)), (StatusCode::OK, Some(0)));
    }

    #[tokio::test]
    async fn bad_paging_is_request_invalid() {
        let app = TestApp::new();

        for query in ["limit=0", "limit=101", "limit=abc", "offset=-1", "page=2"] {
            let (status, body) = app.json(get(&format!("{DOCUMENTS_PATH}?{query}"))).await;
            assert_error(status, &body, StatusCode::BAD_REQUEST, "RequestInvalid");
        }
    }

    #[tokio::test]
    async fn gets_the_document_with_its_reading_position() {
        let app = TestApp::new();
        let id = app.import_book().await;

        let (status, body) = app.json(get(&document_path(&id))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!((body["id"].as_str(), body["cleanerVersion"].as_u64()), (Some(id.as_str()), Some(1)));
        assert_eq!(body["blocks"][0]["sentences"][0]["text"], "Intro");
        assert_eq!(body["toc"], serde_json::json!([{ "id": 0, "title": "Intro", "level": 1, "sentenceId": 0, "pageIndex": 1 }]));
        assert_eq!(body["blocks"][3]["cue"]["id"], sentences::CODE_CUE);
        assert!(body["lastSentenceId"].is_null(), "{body}");

        let (status, _, _) = app.send(put_json(&position_path(&id), r#"{ "sentenceId": 3 }"#)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (_, body) = app.json(get(&document_path(&id))).await;
        assert_eq!(body["lastSentenceId"], 3);
        let (_, list) = app.json(get(DOCUMENTS_PATH)).await;
        assert_eq!(list["items"][0]["lastSentenceId"], 3);
    }

    #[tokio::test]
    async fn unknown_and_malformed_document_ids_are_document_not_found() {
        let app = TestApp::new();

        for id in [UNKNOWN_ID, "not-a-hash", "..%2F..%2Fetc"] {
            let (status, body) = app.json(get(&document_path(id))).await;
            assert_error(status, &body, StatusCode::NOT_FOUND, "DocumentNotFound");
            let (status, body) = app.json(put_json(&position_path(id), r#"{ "sentenceId": 0 }"#)).await;
            assert_error(status, &body, StatusCode::NOT_FOUND, "DocumentNotFound");
        }
    }

    #[tokio::test]
    async fn position_errors() {
        let app = TestApp::new();
        let id = app.import_book().await;

        let (status, body) = app.json(put_json(&position_path(&id), &format!(r#"{{ "sentenceId": {} }}"#, sentences::COUNT))).await;
        assert_error(status, &body, StatusCode::NOT_FOUND, "SentenceNotFound");
        for bad in ["{}", r#"{ "sentenceId": -1 }"#, r#"{ "sentenceId": "3" }"#, "not json"] {
            let (status, body) = app.json(put_json(&position_path(&id), bad)).await;
            assert_error(status, &body, StatusCode::BAD_REQUEST, "RequestInvalid");
        }
    }
}
