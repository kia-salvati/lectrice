//! The HTTP shell over the core: config, logging, routes, error mapping and wiring.

pub mod app;
pub mod composition;
pub mod config;
pub mod context;
pub mod dev_clean;
pub mod dev_speak;
pub mod error;
pub mod logging;
pub mod routes;
pub mod server;
pub mod shutdown;
pub mod state;
#[cfg(test)]
mod test_support;
