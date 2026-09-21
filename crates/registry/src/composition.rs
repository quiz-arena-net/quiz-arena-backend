use opentelemetry::metrics::Meter;
use quiz_arena_shared::config::AppConfig;

use crate::{
    config::{REGISTRY_SECTION_NAME, RegistryConfig},
    presentation::connectrpc::{QuizAuthoringServiceHandler, QuizCatalogServiceHandler},
};

/// The service's ConnectRPC handlers.
pub(crate) struct Handlers {
    pub(crate) quiz_catalog: QuizCatalogServiceHandler,
    pub(crate) quiz_authoring: QuizAuthoringServiceHandler,
}

/// Composition root.
///
/// Wires infrastructure into application use cases and returns the service's
/// ConnectRPC handlers.
///
/// Async and fallible because it connects to external resources. An error
/// aborts startup.
pub(crate) async fn compose(app_config: &mut AppConfig, _meter: Meter) -> anyhow::Result<Handlers> {
    let _config: RegistryConfig = app_config.section(REGISTRY_SECTION_NAME, &[])?;
    Ok(Handlers {
        quiz_catalog: QuizCatalogServiceHandler::new(),
        quiz_authoring: QuizAuthoringServiceHandler::new(),
    })
}
