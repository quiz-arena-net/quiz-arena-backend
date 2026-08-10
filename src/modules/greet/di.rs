use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use sea_orm::{ConnectOptions, Database};

use crate::{Module, ModuleContext, ModuleRegistration, shared::infrastructure::SeaOrmUnitOfWork};

use super::{
    application::GreetInteractor,
    config::GreetConfig,
    infrastructure::{self, SeaOrmGreetTransactionContextFactory},
    presentation::connectrpc::GreetServiceHandler,
};

/// Composition root: wires infrastructure into application use cases and
/// returns the module's ConnectRPC handler.
async fn init(config: GreetConfig) -> anyhow::Result<GreetServiceHandler> {
    let mut connect_options = ConnectOptions::new(&config.database_url)
        .sqlx_logging_level(log::LevelFilter::Debug)
        .sqlx_slow_statements_logging_settings(log::LevelFilter::Warn, Duration::from_secs(1))
        .to_owned();
    if config.database_url.starts_with("sqlite") {
        connect_options.idle_timeout(None).max_lifetime(None);
    }
    let database = Database::connect(connect_options).await?;
    infrastructure::create_schema(&database).await?;
    let unit_of_work = SeaOrmUnitOfWork::new(database, SeaOrmGreetTransactionContextFactory);
    let greet_usecase = Arc::new(GreetInteractor::new(unit_of_work));
    Ok(GreetServiceHandler::new(greet_usecase))
}

/// Greets senders and remembers who has greeted before.
pub(super) struct GreetModule;

#[async_trait]
impl Module for GreetModule {
    const NAME: &'static str = "greet";
    type Config = GreetConfig;

    fn service_names(&self) -> &'static [&'static str] {
        &[crate::proto::quiz_arena::greet::v1::GREET_SERVICE_SERVICE_NAME]
    }

    async fn register(
        &self,
        mut ctx: ModuleContext,
        config: GreetConfig,
    ) -> anyhow::Result<ModuleContext> {
        ctx.connect_router = ctx
            .connect_router
            .add_service(Arc::new(init(config).await?));
        Ok(ctx)
    }
}

inventory::submit! {
    ModuleRegistration(&GreetModule)
}
