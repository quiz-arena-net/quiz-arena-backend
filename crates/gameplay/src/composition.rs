use opentelemetry::metrics::Meter;
use quiz_arena_shared::config::AppConfig;

use crate::{
    config::{GAMEPLAY_SECTION_NAME, GameplayConfig},
    presentation::connectrpc::GameplayServiceHandler,
};

/// Composition root.
///
/// Wires infrastructure into application use cases and returns the service's
/// ConnectRPC handler.
///
/// Async and fallible because it connects to external resources. An error
/// aborts startup.
pub(crate) async fn compose(
    app_config: &mut AppConfig,
    _meter: Meter,
) -> anyhow::Result<GameplayServiceHandler> {
    let _config: GameplayConfig = app_config.section(GAMEPLAY_SECTION_NAME, &[])?;
    Ok(GameplayServiceHandler::new())
}
