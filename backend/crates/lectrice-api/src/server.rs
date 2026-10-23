use std::net::Ipv4Addr;

use tokio::net::TcpListener;

use crate::app::build_app;
use crate::config::Config;
use crate::shutdown::signal_received;
use crate::state::AppState;

const SERVICE: &str = "server";

/// Local app: only listens on the loopback interface.
const BIND_ADDRESS: Ipv4Addr = Ipv4Addr::LOCALHOST;

pub async fn serve(config: &Config, state: AppState) -> std::io::Result<()> {
    let listener = TcpListener::bind((BIND_ADDRESS, config.server.port)).await?;
    tracing::info!(service = SERVICE, address = %BIND_ADDRESS, port = config.server.port, "Server listening");

    let app = build_app(state, config.server.allowed_origin.clone());
    axum::serve(listener, app).with_graceful_shutdown(signal_received()).await?;
    tracing::info!(service = SERVICE, "Server stopped");
    Ok(())
}
