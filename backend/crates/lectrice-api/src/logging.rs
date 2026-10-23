//! One JSON object per line (spec §9). Log ids, counts, durations, writes and errors, never book content.

use axum::http::Request;
use tracing::Span;
use tracing_subscriber::EnvFilter;

use crate::context::REQUEST_ID_HEADER;

const DEFAULT_LOG_FILTER: &str = "info";

/// `RUST_LOG` overrides the filter.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    tracing_subscriber::fmt().json().flatten_event(true).with_current_span(true).with_span_list(false).with_env_filter(filter).init();
}

/// The HTTP request span: method, path (no query, headers or body) and request id.
pub fn request_span<B>(request: &Request<B>) -> Span {
    let request_id = request.headers().get(REQUEST_ID_HEADER).and_then(|value| value.to_str().ok()).unwrap_or_default();
    tracing::info_span!("request", method = %request.method(), path = request.uri().path(), request_id)
}
