//! Input helpers shared by the routes: typed query and JSON extractors whose rejections use the error
//! registry, path-id parsing, and running a (synchronous) service off the async runtime.

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Query, Request};
use axum::http::request::Parts;
use lectrice_app::Cancellation;
use lectrice_domain::{AppError, AppResult, ErrorName, SentenceId};
use serde::de::DeserializeOwned;

use crate::error::ApiError;

/// `Query<T>` that rejects with `RequestInvalid`.
pub struct ValidQuery<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequestParts<S> for ValidQuery<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state).await.map_err(|rejection: QueryRejection| invalid(rejection.body_text()))?;
        Ok(Self(value))
    }
}

/// `Json<T>` that rejects with `RequestInvalid`.
pub struct ValidJson<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for ValidJson<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state).await.map_err(|rejection: JsonRejection| invalid(rejection.body_text()))?;
        Ok(Self(value))
    }
}

pub fn invalid(detail: impl Into<String>) -> ApiError {
    ApiError(AppError::new(ErrorName::RequestInvalid).with_detail(detail))
}

/// Like `DocumentId::parse` (a malformed id is `DocumentNotFound`), a malformed sentence id can't name a sentence, so it is `SentenceNotFound`.
pub fn sentence_id(raw: &str) -> AppResult<SentenceId> {
    raw.trim().parse().map(SentenceId).map_err(|_| AppError::new(ErrorName::SentenceNotFound).with_detail("sentence id is not a number"))
}

/// Cancels a request's context when dropped. axum drops a handler's future when the client disconnects,
/// but the blocking task it started keeps running; this tells that task to stop waiting. Dropping it
/// after the work is done is harmless, because nothing checks the flag any more.
pub struct CancelOnDrop(pub Cancellation);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// Services are synchronous (disk I/O, JSON, waiting for speech), so they run on the blocking pool.
pub async fn run_blocking<T: Send + 'static>(work: impl FnOnce() -> AppResult<T> + Send + 'static) -> Result<T, ApiError> {
    let joined = tokio::task::spawn_blocking(work).await;
    let result = joined.map_err(|error| AppError::new(ErrorName::Internal).with_detail("blocking service task failed").with_source(error))?;
    Ok(result?)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn dropping_the_handler_future_cancels_the_request() {
        let cancellation = Cancellation::default();
        let handler = {
            let cancellation = cancellation.clone();
            async move {
                let _cancel_on_disconnect = CancelOnDrop(cancellation);
                std::future::pending::<()>().await;
            }
        };

        assert!(tokio::time::timeout(Duration::from_millis(10), handler).await.is_err());
        assert!(cancellation.is_cancelled());
    }
}
