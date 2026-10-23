use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Per-call data every service receives first. Carries no book content.
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: String,
    /// Set when the caller has given up (e.g. the HTTP client disconnected). Services that wait may stop early.
    pub cancellation: Cancellation,
}

impl RequestContext {
    pub fn new(request_id: impl Into<String>) -> Self {
        Self { request_id: request_id.into(), cancellation: Cancellation::default() }
    }
}

/// A flag shared between the caller and the service working for it.
#[derive(Debug, Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
