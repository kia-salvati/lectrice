use std::future::pending;

use tokio::signal::unix::{SignalKind, signal};

const SERVICE: &str = "server";

/// Resolves on SIGINT or SIGTERM. axum then stops accepting connections and drains open requests.
pub async fn signal_received() {
    tokio::select! {
        () = interrupt() => {}
        () = terminate() => {}
    }
    tracing::info!(service = SERVICE, "Shutdown signal received, draining requests");
}

async fn interrupt() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(service = SERVICE, error = %error, "Cannot listen for SIGINT");
        pending::<()>().await;
    }
}

async fn terminate() {
    match signal(SignalKind::terminate()) {
        Ok(mut stream) => {
            stream.recv().await;
        }
        Err(error) => {
            tracing::error!(service = SERVICE, error = %error, "Cannot listen for SIGTERM");
            pending::<()>().await;
        }
    }
}
