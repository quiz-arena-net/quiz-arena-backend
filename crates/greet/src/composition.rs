use std::{sync::Arc, time::Duration};

use opentelemetry::metrics::Meter;
use quiz_arena_shared::{config::AppConfig, kernel::infrastructure::SeaOrmUnitOfWork};
use sea_orm::{ConnectOptions, Database};

use crate::{
    application::GreetInteractor,
    config::{GREET_SECTION_NAME, GreetConfig},
    infrastructure::{SeaOrmGreetTransactionContextFactory, create_schema},
    presentation::connectrpc::GreetServiceHandler,
};

/// Composition root.
///
/// Wires infrastructure into application use cases and returns the service's
/// ConnectRPC handler.
///
/// Async and fallible because it connects to external resources. An error
/// aborts startup.
pub(crate) async fn compose(
    app_config: &AppConfig,
    meter: Meter,
) -> anyhow::Result<GreetServiceHandler> {
    let config: GreetConfig = app_config.section(GREET_SECTION_NAME, &[])?;

    let mut connect_options = ConnectOptions::new(&config.database_url)
        .sqlx_logging_level(log::LevelFilter::Debug)
        .sqlx_slow_statements_logging_settings(log::LevelFilter::Warn, Duration::from_secs(1))
        .to_owned();
    if config.database_url.starts_with("sqlite") {
        connect_options.idle_timeout(None).max_lifetime(None);
    }
    let database = Database::connect(connect_options).await?;
    create_schema(&database).await?;
    let unit_of_work = SeaOrmUnitOfWork::new(database, SeaOrmGreetTransactionContextFactory);
    let greet_usecase = Arc::new(GreetInteractor::new(unit_of_work, meter));

    Ok(GreetServiceHandler::new(greet_usecase))
}
