//! Who may call the local web server (security review S2). Binding to loopback keeps other machines out, but not
//! web pages in the user's own browser: a page on any site can send requests to `localhost`, and with DNS rebinding
//! (the attacker's host name answering `127.0.0.1`) it can read the answers too. CORS only adds response headers; it
//! never refuses a request. So every request is checked here before a route runs:
//!
//! - `Host` must name this server on loopback (`localhost`, `127.0.0.1` or `[::1]` with the server's port). A
//!   rebound page sends its own host name, so it is refused.
//! - `Origin`, when sent, must be the configured page or this server itself.
//! - A request that changes something (anything but `GET`, `HEAD`, `OPTIONS`) must carry such an `Origin`. Browsers
//!   always send it on those, so a cross-site form or upload can't get in by leaving it out.
//! - A browser request marked `Sec-Fetch-Site: cross-site` needs an allowed `Origin` too (cross-site `<audio>`,
//!   `<img>` and link navigations carry none).

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::{HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use lectrice_domain::{AppError, ErrorName};

use crate::error::ApiError;

const LOOPBACK_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];
const HTTP_DEFAULT_PORT: u16 = 80;
const SEC_FETCH_SITE: HeaderName = HeaderName::from_static("sec-fetch-site");
const CROSS_SITE: &str = "cross-site";
/// Header values in the log are cut to this many characters.
const LOGGED_VALUE_CHARS: usize = 80;

/// The hosts and origins the server answers to.
#[derive(Debug, Clone)]
pub struct LocalAccess {
    hosts: Vec<String>,
    origins: Vec<String>,
}

impl LocalAccess {
    /// For the server on loopback `port`, with the web page at `allowed_origin`.
    pub fn new(port: u16, allowed_origin: &HeaderValue) -> Self {
        let mut hosts: Vec<String> = LOOPBACK_HOSTS.iter().map(|host| format!("{host}:{port}")).collect();
        if port == HTTP_DEFAULT_PORT {
            // Browsers leave the default port out.
            hosts.extend(LOOPBACK_HOSTS.iter().map(|host| (*host).to_string()));
        }
        let mut origins: Vec<String> = hosts.iter().map(|host| format!("http://{host}")).collect();
        if let Ok(page) = allowed_origin.to_str().map(str::to_ascii_lowercase) {
            // A dev server on this machine that proxies `/api` here (Vite) passes the page's own `Host` on. A page
            // elsewhere can't make a browser send that name to this port, so it is as safe as the server's own.
            hosts.extend(loopback_authority(&page).map(str::to_string));
            origins.push(page);
        }
        Self { hosts, origins }
    }

    /// `Err` carries the reason for the log.
    fn check(&self, method: &Method, uri_authority: Option<&str>, headers: &HeaderMap) -> Result<(), String> {
        let host = match single(headers, &HOST)? {
            Some(host) => host,
            // HTTP/2 names the host in the request target instead.
            None => uri_authority.ok_or("no Host header")?.to_ascii_lowercase(),
        };
        if !self.hosts.contains(&host) {
            return Err(format!("host {:?} is not this server", shortened(&host)));
        }
        let origin = single(headers, &ORIGIN)?;
        if let Some(origin) = &origin
            && !self.origins.contains(origin)
        {
            return Err(format!("origin {:?} is not allowed", shortened(origin)));
        }
        if origin.is_none() {
            if !is_read_only(method) {
                return Err(format!("{method} without an Origin header"));
            }
            if single(headers, &SEC_FETCH_SITE)?.as_deref() == Some(CROSS_SITE) {
                return Err("a cross-site request without an Origin header".to_string());
            }
        }
        Ok(())
    }
}

/// `host[:port]` of an `http(s)://` origin whose host is a loopback name.
fn loopback_authority(origin: &str) -> Option<&str> {
    let authority = origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://"))?;
    LOOPBACK_HOSTS
        .iter()
        .any(|host| authority.strip_prefix(host).is_some_and(|rest| rest.is_empty() || rest.strip_prefix(':').is_some_and(is_port)))
        .then_some(authority)
}

fn is_port(text: &str) -> bool {
    text.parse::<u16>().is_ok()
}

fn is_read_only(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// The header's value in lower case; an error when it is sent twice or isn't text (two values could be read
/// differently here and by whatever sits behind).
fn single(headers: &HeaderMap, name: &HeaderName) -> Result<Option<String>, String> {
    let mut values = headers.get_all(name).iter();
    let Some(value) = values.next() else { return Ok(None) };
    if values.next().is_some() {
        return Err(format!("more than one {name} header"));
    }
    value.to_str().map(|text| Some(text.to_ascii_lowercase())).map_err(|_| format!("the {name} header isn't text"))
}

fn shortened(value: &str) -> String {
    value.chars().take(LOGGED_VALUE_CHARS).collect()
}

/// The middleware: refuses with `RequestForbidden` (403) before any route, extractor or body read.
pub async fn enforce(State(access): State<Arc<LocalAccess>>, request: Request, next: Next) -> Response {
    let authority = request.uri().authority().map(|authority| authority.as_str());
    match access.check(request.method(), authority, request.headers()) {
        Ok(()) => next.run(request).await,
        Err(detail) => ApiError(AppError::new(ErrorName::RequestForbidden).with_detail(detail)).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};

    use super::*;
    use crate::app::build_local_app;
    use crate::routes;
    use crate::test_support::{PDF_BYTES, TestApp, error_name, upload};

    const PORT: u16 = 7878;
    const PAGE: &str = "http://localhost:5173";
    const SERVER: &str = "localhost:7878";

    /// The test app as `server::serve` builds it.
    fn guarded() -> TestApp {
        let mut app = TestApp::new();
        app.router = build_local_app(app.state.clone(), HeaderValue::from_static(PAGE), LocalAccess::new(PORT, &HeaderValue::from_static(PAGE)));
        app
    }

    fn request(method: Method, path: &str, headers: &[(&str, &str)]) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(path);
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        builder.body(Body::empty()).unwrap()
    }

    async fn status_of(app: &TestApp, request: Request<Body>) -> (StatusCode, String) {
        let (status, body) = app.json(request).await;
        (status, error_name(&body).to_string())
    }

    fn forbidden() -> (StatusCode, String) {
        (StatusCode::FORBIDDEN, "RequestForbidden".to_string())
    }

    fn with(mut request: Request<Body>, headers: &[(&'static str, &'static str)]) -> Request<Body> {
        for (name, value) in headers {
            request.headers_mut().append(*name, HeaderValue::from_static(value));
        }
        request
    }

    #[tokio::test]
    async fn the_page_and_plain_local_reads_are_served() {
        let app = guarded();
        for headers in [
            vec![("host", SERVER)],
            vec![("host", "127.0.0.1:7878")],
            vec![("host", "[::1]:7878")],
            vec![("host", "LOCALHOST:7878")],
            vec![("host", SERVER), ("origin", PAGE)],
            vec![("host", SERVER), ("origin", "http://localhost:7878")],
            vec![("host", SERVER), ("sec-fetch-site", "same-site")],
            vec![("host", SERVER), ("sec-fetch-site", "none")],
        ] {
            let (status, _) = app.json(request(Method::GET, routes::HEALTH_PATH, &headers)).await;
            assert_eq!(status, StatusCode::OK, "{headers:?}");
        }
    }

    #[tokio::test]
    async fn a_rebound_host_name_is_refused_on_every_route() {
        let app = guarded();
        let id = "a".repeat(64);
        let document = format!("/api/documents/{id}");
        for host in [
            "evil.example:7878",
            "evil.example",
            "localhost:9999",
            "localhost",
            "localhost:5174",
            "127.0.0.1.evil.example:7878",
            "localhost:7878.evil.example",
            "",
        ] {
            for path in [routes::HEALTH_PATH, routes::DOCUMENTS_PATH, document.as_str(), "/nowhere"] {
                assert_eq!(status_of(&app, request(Method::GET, path, &[("host", host)])).await, forbidden(), "{host} {path}");
            }
            // The attacker's page is same-origin with its own host name, so its Origin matches its Host.
            let rebound = request(Method::DELETE, &document, &[("host", host), ("origin", "http://evil.example:7878")]);
            assert_eq!(status_of(&app, rebound).await, forbidden(), "{host}");
        }
    }

    #[tokio::test]
    async fn a_missing_or_doubled_host_is_refused() {
        let app = guarded();
        assert_eq!(status_of(&app, request(Method::GET, routes::HEALTH_PATH, &[])).await, forbidden());
        let doubled = request(Method::GET, routes::HEALTH_PATH, &[("host", SERVER), ("host", "evil.example")]);
        assert_eq!(status_of(&app, doubled).await, forbidden());
    }

    #[tokio::test]
    async fn http2_requests_are_checked_by_their_authority() {
        let app = guarded();
        let (status, _) = app.json(request(Method::GET, "http://localhost:7878/api/health", &[])).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(status_of(&app, request(Method::GET, "http://evil.example:7878/api/health", &[])).await, forbidden());
    }

    #[tokio::test]
    async fn a_cross_site_upload_is_refused_and_nothing_is_imported() {
        let app = guarded();
        for origin in [Some("https://evil.example"), Some("http://localhost:5174"), Some("http://localhost.evil.example:5173"), Some("null"), None] {
            let mut upload = with(upload("Book.pdf", PDF_BYTES), &[("host", SERVER)]);
            if let Some(origin) = origin {
                upload.headers_mut().insert(header::ORIGIN, HeaderValue::from_static(origin));
            }
            assert_eq!(status_of(&app, upload).await, forbidden(), "{origin:?}");
        }
        let (status, body) = app.json(request(Method::GET, routes::DOCUMENTS_PATH, &[("host", SERVER)])).await;
        assert_eq!((status, body["total"].as_u64()), (StatusCode::OK, Some(0)), "{body}");

        let (status, _) = app.json(with(upload("Book.pdf", PDF_BYTES), &[("host", SERVER), ("origin", PAGE)])).await;
        assert_eq!(status, StatusCode::CREATED, "the page itself still uploads");
    }

    #[tokio::test]
    async fn state_changing_requests_need_the_pages_origin() {
        let app = guarded();
        let id = "a".repeat(64);
        let position = format!("/api/documents/{id}/position");
        let exports = format!("/api/documents/{id}/exports");
        let document = format!("/api/documents/{id}");
        for (method, path) in [(Method::PUT, position.as_str()), (Method::POST, exports.as_str()), (Method::DELETE, document.as_str())] {
            assert_eq!(status_of(&app, request(method.clone(), path, &[("host", SERVER)])).await, forbidden(), "{method} without Origin");
            let foreign = request(method.clone(), path, &[("host", SERVER), ("origin", "https://evil.example")]);
            assert_eq!(status_of(&app, foreign).await, forbidden(), "{method} cross-site");
            let (status, _) = status_of(&app, request(method.clone(), path, &[("host", SERVER), ("origin", PAGE)])).await;
            assert_ne!(status, StatusCode::FORBIDDEN, "{method} from the page reaches its route");
        }
    }

    #[tokio::test]
    async fn cross_site_reads_without_an_origin_are_refused() {
        let app = guarded();
        let embedded = request(Method::GET, routes::HEALTH_PATH, &[("host", SERVER), ("sec-fetch-site", "cross-site")]);
        assert_eq!(status_of(&app, embedded).await, forbidden());
        let foreign = request(Method::GET, routes::HEALTH_PATH, &[("host", SERVER), ("origin", "https://evil.example")]);
        assert_eq!(status_of(&app, foreign).await, forbidden());
        let doubled = request(Method::GET, routes::HEALTH_PATH, &[("host", SERVER), ("origin", PAGE), ("origin", "https://evil.example")]);
        assert_eq!(status_of(&app, doubled).await, forbidden());
    }

    #[tokio::test]
    async fn preflights_pass_for_the_page_only() {
        let app = guarded();
        let preflight = |origin: &'static str| {
            request(Method::OPTIONS, routes::DOCUMENTS_PATH, &[("host", SERVER), ("origin", origin), ("access-control-request-method", "POST")])
        };
        let (status, headers, _) = app.send(preflight(PAGE)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], PAGE);
        let (status, headers, _) = app.send(preflight("https://evil.example")).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    }

    #[tokio::test]
    async fn a_refusal_has_the_registry_shape_and_a_request_id() {
        let (status, headers, body) = guarded().send(request(Method::GET, routes::HEALTH_PATH, &[("host", "evil.example:7878")])).await;
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!((status, body["error"]["code"].as_u64()), (StatusCode::FORBIDDEN, Some(1009)));
        assert!(headers.contains_key(crate::context::REQUEST_ID_HEADER));
    }

    #[test]
    fn the_default_http_port_also_matches_without_a_port() {
        let access = LocalAccess::new(80, &HeaderValue::from_static(PAGE));
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("localhost"));
        assert!(access.check(&Method::GET, None, &headers).is_ok());
        headers.insert(HOST, HeaderValue::from_static("evil.example"));
        assert!(access.check(&Method::GET, None, &headers).is_err());
    }

    #[tokio::test]
    async fn a_local_dev_server_that_proxies_with_the_pages_host_is_served() {
        let app = guarded();
        let (status, _) = app.json(request(Method::GET, routes::HEALTH_PATH, &[("host", "localhost:5173")])).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = app.json(with(upload("Book.pdf", PDF_BYTES), &[("host", "localhost:5173"), ("origin", PAGE)])).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    #[test]
    fn only_a_loopback_page_adds_its_host() {
        for (origin, added) in [
            ("http://localhost:5173", Some("localhost:5173")),
            ("https://127.0.0.1:8443", Some("127.0.0.1:8443")),
            ("http://[::1]:5173", Some("[::1]:5173")),
            ("http://localhost", Some("localhost")),
            ("https://lectrice.example", None),
            ("http://localhost.evil.example:5173", None),
            ("http://localhost:5173.evil.example", None),
            ("http://localhost:5173@evil.example", None),
            ("localhost:5173", None),
        ] {
            assert_eq!(loopback_authority(origin), added, "{origin}");
        }
    }
}
