//! The greet service.
//!
//! Greets senders and remembers who has greeted before.

mod application;
mod composition;
mod config;
mod domain;
mod infrastructure;
mod presentation;

use std::sync::Arc;

use connectrpc::Router as ConnectRouter;
use quiz_arena_shared::{config::AppConfig, server, telemetry};
use tracing::info;

use crate::composition::compose;

const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app_config = AppConfig::load()?;
    let telemetry = telemetry::init(app_config.telemetry()?, SERVICE_NAME)?;

    info!(
        "Starting {SERVICE_NAME} {}",
        quiz_arena_shared::BUILD_VERSION
    );

    if telemetry.is_exporting() {
        info!("Telemetry export enabled");
    } else {
        info!("Telemetry export disabled, stdout only");
    }

    let server_config = app_config.server()?;
    let handler = compose(&app_config, telemetry.meter()).await?;

    server::serve(
        ConnectRouter::new().add_service(Arc::new(handler)),
        vec![quiz_arena_proto::greet::v1::GREET_SERVICE_SERVICE_NAME],
        quiz_arena_proto::FILE_DESCRIPTOR_SET,
        server_config,
        telemetry,
    )
    .await
}
