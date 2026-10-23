use axum::Router;
use axum::http::{HeaderName, HeaderValue, Method, header};
use tower_http::LatencyUnit;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::context::REQUEST_ID_HEADER;
use crate::logging::request_span;
use crate::routes;
use crate::state::AppState;

const CORS_METHODS: [Method; 4] = [Method::GET, Method::POST, Method::PUT, Method::DELETE];

/// The full HTTP app: routes plus request id, request logging and CORS.
pub fn build_app(state: AppState, allowed_origin: HeaderValue) -> Router {
    let request_id_header = HeaderName::from_static(REQUEST_ID_HEADER);
    let trace =
        TraceLayer::new_for_http().make_span_with(request_span).on_response(DefaultOnResponse::new().level(Level::INFO).latency_unit(LatencyUnit::Millis));
    let cors = CorsLayer::new().allow_origin(allowed_origin).allow_methods(CORS_METHODS).allow_headers([header::CONTENT_TYPE]);

    routes::routes()
        .with_state(state)
        .layer(cors)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(trace)
        .layer(SetRequestIdLayer::new(request_id_header, MakeRequestUuid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{TestApp, get};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::json;

    #[tokio::test]
    async fn health_reports_a_ready_engine() {
        let (status, body) = TestApp::new().json(get(routes::HEALTH_PATH)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "status": "ok", "speechEngine": "ready" }));
    }

    #[tokio::test]
    async fn health_reports_a_failed_engine() {
        let (status, body) = TestApp::without_engine().json(get(routes::HEALTH_PATH)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "status": "ok", "speechEngine": "failed" }));
    }

    #[tokio::test]
    async fn every_response_carries_a_request_id() {
        let (_, headers, _) = TestApp::new().send(get(routes::HEALTH_PATH)).await;
        assert!(headers.contains_key(REQUEST_ID_HEADER));
    }

    #[tokio::test]
    async fn a_client_request_id_is_kept() {
        let request = Request::builder().uri(routes::HEALTH_PATH).header(REQUEST_ID_HEADER, "abc-123").body(Body::empty()).unwrap();
        let (_, headers, _) = TestApp::new().send(request).await;
        assert_eq!(headers[REQUEST_ID_HEADER], "abc-123");
    }
}
