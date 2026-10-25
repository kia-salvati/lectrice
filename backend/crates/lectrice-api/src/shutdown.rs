use std::future::pending;

const SERVICE: &str = "server";

/// Resolves on Ctrl+C (SIGINT), or SIGTERM on Unix. axum then stops accepting connections and drains open requests.
pub async fn signal_received() {
    tokio::select! {
        () = interrupt() => {}
        () = terminate() => {}
    }
    tracing::info!(service = SERVICE, "Shutdown signal received, draining requests");
}

async fn interrupt() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(service = SERVICE, error = %error, "Cannot listen for Ctrl+C");
        pending::<()>().await;
    }
}

#[cfg(unix)]
async fn terminate() {
    use tokio::signal::unix::{SignalKind, signal};

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

#[cfg(not(unix))]
async fn terminate() {
    pending::<()>().await;
}
