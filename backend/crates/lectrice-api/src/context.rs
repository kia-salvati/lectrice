use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use lectrice_app::RequestContext;

/// Set on every request by the request-id layer (see `app::build_app`).
pub const REQUEST_ID_HEADER: &str = "x-request-id";

const MISSING_REQUEST_ID: &str = "none";

/// Extracts the [`RequestContext`] a controller passes first to its service.
#[derive(Debug, Clone)]
pub struct Ctx(pub RequestContext);

impl<S: Send + Sync> FromRequestParts<S> for Ctx {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(RequestContext::new(request_id(parts))))
    }
}

fn request_id(parts: &Parts) -> &str {
    parts.headers.get(REQUEST_ID_HEADER).and_then(|value| value.to_str().ok()).unwrap_or(MISSING_REQUEST_ID)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    async fn extract(request: Request<()>) -> RequestContext {
        let (mut parts, ()) = request.into_parts();
        let Ok(Ctx(ctx)) = Ctx::from_request_parts(&mut parts, &()).await;
        ctx
    }

    #[tokio::test]
    async fn reads_the_request_id_header() {
        let request = Request::builder().header(REQUEST_ID_HEADER, "req-7").body(()).unwrap();
        assert_eq!(extract(request).await.request_id, "req-7");
    }

    #[tokio::test]
    async fn falls_back_when_the_header_is_missing() {
        assert_eq!(extract(Request::new(())).await.request_id, MISSING_REQUEST_ID);
    }
}
