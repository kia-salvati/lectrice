use std::path::PathBuf;
use std::process::ExitCode;

use lectrice_api::config::{self, CONFIG_PATH_ENV, DEFAULT_CONFIG_PATH};
use lectrice_api::{composition, dev_clean, dev_speak, logging, server};

const SERVICE: &str = "main";

#[tokio::main]
async fn main() -> ExitCode {
    logging::init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(service = SERVICE, error = %error, cause = ?error.source(), "Lectrice stopped with an error");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = std::env::var_os(CONFIG_PATH_ENV).map_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH), PathBuf::from);
    let config = config::load(&config_path, std::env::vars(), std::env::home_dir().as_deref())?;
    tracing::info!(service = SERVICE, core_version = lectrice_app::CORE_VERSION, config_path = %config_path.display(), port = config.server.port, data_dir = %config.storage.data_dir.display(), "Config loaded");

    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some(dev_speak::COMMAND) => return dev_speak::run(&config.speech, args),
        Some(dev_clean::COMMAND) => return dev_clean::run(&config.pdf, args),
        _ => {}
    }

    let state = composition::build_state(&config)?;
    server::serve(&config, state).await?;
    Ok(())
}
