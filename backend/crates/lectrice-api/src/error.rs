use std::error::Error as StdError;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use lectrice_domain::{AppError, ErrorSpec};
use serde::Serialize;

const SERVICE: &str = "http";
const CAUSE_SEPARATOR: &str = " <- ";

/// The only place an error name becomes HTTP (spec §7, §8). Clients get the registry's message;
/// the detail and cause chain go to the log only.
#[derive(Debug)]
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        Self(error)
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorPayload,
}

#[derive(Serialize)]
struct ErrorPayload {
    name: &'static str,
    code: u32,
    message: &'static str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let name = self.0.primary();
        let spec = name.spec();
        log_error(&self.0, spec);
        let status = StatusCode::from_u16(spec.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = ErrorBody { error: ErrorPayload { name: name.as_str(), code: spec.code, message: spec.description } };
        (status, Json(body)).into_response()
    }
}

fn log_error(error: &AppError, spec: ErrorSpec) {
    let names: Vec<&str> = error.names().iter().map(|name| name.as_str()).collect();
    let names = names.join(",");
    let detail = error.detail().unwrap_or_default();
    let cause = cause_chain(error);
    if spec.operational {
        tracing::warn!(service = SERVICE, error.names = names, error.code = spec.code, detail, cause, "Request failed");
    } else {
        tracing::error!(service = SERVICE, error.names = names, error.code = spec.code, detail, cause, "Request failed unexpectedly");
    }
}

fn cause_chain(error: &AppError) -> String {
    let mut causes = Vec::new();
    let mut next = error.source();
    while let Some(cause) = next {
        causes.push(cause.to_string());
        next = cause.source();
    }
    causes.join(CAUSE_SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;
    use lectrice_domain::ErrorName;
    use serde_json::{Value, json};

    async fn render(error: AppError) -> (StatusCode, Value) {
        let response = ApiError(error).into_response();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn operational_error_has_the_spec_shape() {
        let (status, body) = render(AppError::new(ErrorName::PdfNoText)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body, json!({ "error": { "name": "PdfNoText", "code": 1005, "message": "This PDF has no selectable text." } }));
    }

    #[tokio::test]
    async fn non_operational_error_hides_its_detail_and_cause() {
        let cause = std::io::Error::other("/home/reader/.local/share/lectrice: permission denied");
        let error = AppError::new(ErrorName::StorageFailed).with_detail("writing document.json").with_source(cause);
        let (status, body) = render(error).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, json!({ "error": { "name": "StorageFailed", "code": 9001, "message": "Something went wrong saving data." } }));
    }

    #[tokio::test]
    async fn the_primary_name_decides_the_response() {
        let (status, body) = render(AppError::new(ErrorName::VoiceUnknown).and(ErrorName::SentenceNotFound)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["name"], "VoiceUnknown");
    }
}
