//! Route table (spec §7). Each feature file holds its input DTOs (with validation), presenters and thin
//! controllers.

mod audio;
mod documents;
mod exports;
mod extract;
mod health;
mod voices;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{Method, Uri};
use axum::routing::{get, put};
use lectrice_domain::{AppError, ErrorName};

use crate::error::ApiError;
use crate::state::AppState;

pub const HEALTH_PATH: &str = "/api/health";
pub const VOICES_PATH: &str = "/api/voices";
pub const DOCUMENTS_PATH: &str = "/api/documents";
pub const DOCUMENT_PATH: &str = "/api/documents/{id}";
pub const POSITION_PATH: &str = "/api/documents/{id}/position";
pub const SENTENCE_AUDIO_PATH: &str = "/api/documents/{id}/sentences/{sentenceId}/audio";
pub const DOCUMENT_EXPORTS_PATH: &str = "/api/documents/{id}/exports";
pub const EXPORT_PATH: &str = "/api/exports/{exportId}";
pub const EXPORT_FILE_PATH: &str = "/api/exports/{exportId}/file";

pub fn routes() -> Router<AppState> {
    // The upload controller enforces `upload.max_mb` itself, so the error has the registry shape.
    let documents = get(documents::list_documents).post(documents::import_pdf).layer(DefaultBodyLimit::disable());
    Router::new()
        .route(HEALTH_PATH, get(health::get_health))
        .route(VOICES_PATH, get(voices::list_voices))
        .route(DOCUMENTS_PATH, documents)
        .route(DOCUMENT_PATH, get(documents::get_document))
        .route(POSITION_PATH, put(documents::save_position))
        .route(SENTENCE_AUDIO_PATH, get(audio::get_sentence_audio))
        .route(DOCUMENT_EXPORTS_PATH, get(exports::list_exports).post(exports::start_export))
        .route(EXPORT_PATH, get(exports::get_export).delete(exports::cancel_export))
        .route(EXPORT_FILE_PATH, get(exports::download_export))
        .fallback(route_not_found)
        // Last: it only covers the routes added before it.
        .method_not_allowed_fallback(method_not_allowed)
}

/// Unknown paths get the registry's error shape (spec §7), not axum's empty 404.
async fn route_not_found(method: Method, uri: Uri) -> ApiError {
    ApiError(AppError::new(ErrorName::RouteNotFound).with_detail(format!("{method} {}", uri.path())))
}

/// A known path with a method it doesn't accept: the registry's 405 instead of axum's empty one.
async fn method_not_allowed(method: Method, uri: Uri) -> ApiError {
    ApiError(AppError::new(ErrorName::MethodNotAllowed).with_detail(format!("{method} {}", uri.path())))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};

    use super::*;
    use crate::test_support::{TestApp, error_name, get};

    #[tokio::test]
    async fn an_unknown_path_is_route_not_found_in_the_error_shape() {
        for path in ["/api/nope", "/", "/api/documents/abc/unknown"] {
            let (status, body) = TestApp::new().json(get(path)).await;
            assert_eq!((status, error_name(&body)), (StatusCode::NOT_FOUND, "RouteNotFound"), "{path}");
            assert_eq!(body["error"]["code"], 1006);
        }
    }

    #[tokio::test]
    async fn a_wrong_method_is_method_not_allowed_in_the_error_shape() {
        let request = Request::builder().method(Method::DELETE).uri(HEALTH_PATH).body(Body::empty()).unwrap();
        let (status, body) = TestApp::new().json(request).await;
        assert_eq!((status, error_name(&body)), (StatusCode::METHOD_NOT_ALLOWED, "MethodNotAllowed"));
        assert_eq!(body["error"]["code"], 1007);
    }
}
