use async_trait::async_trait;
use connectrpc::Router as ConnectRouter;
use serde::{Serialize, de::DeserializeOwned};

use crate::config::AppConfig;

/// A self-contained feature module.
///
/// Implementations own their composition root: they assemble their layers
/// internally and expose nothing but what the server needs to mount them.
#[async_trait]
pub(crate) trait Module: Sync {
    /// Module name.
    ///
    /// By construction also the module's config section name and environment
    /// variable segment (`QUIZ_ARENA_<NAME>_<FIELD>`).
    const NAME: &'static str;

    /// Shape of this module's config section.
    ///
    /// Extracted and validated by the registry before
    /// [`register`](Module::register) runs. Use `()` for modules without
    /// config.
    type Config: DeserializeOwned + Serialize + Default + Send;

    /// Full service names this module serves, for health reporting.
    fn service_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Assembles the module and registers its services on the context.
    ///
    /// Async and fallible because composition roots connect to external
    /// resources (databases, ...). An error aborts startup.
    async fn register(
        &self,
        ctx: ModuleContext,
        _config: Self::Config,
    ) -> anyhow::Result<ModuleContext> {
        Ok(ctx)
    }
}

/// Object-safe view of [`Module`] for the registry.
///
/// The associated const and type make [`Module`] itself not dyn compatible, so
/// registrations hold this instead. Blanket-implemented for every [`Module`],
/// never implemented by hand.
#[async_trait]
pub(crate) trait DynModule: Sync {
    /// [`Module::NAME`].
    fn name(&self) -> &'static str;

    /// [`Module::service_names`].
    fn service_names(&self) -> &'static [&'static str];

    /// Extracts the config section named [`Module::NAME`] and delegates to
    /// [`Module::register`].
    async fn register(&self, ctx: ModuleContext) -> anyhow::Result<ModuleContext>;
}

#[async_trait]
impl<M: Module> DynModule for M {
    fn name(&self) -> &'static str {
        M::NAME
    }

    fn service_names(&self) -> &'static [&'static str] {
        Module::service_names(self)
    }

    async fn register(&self, ctx: ModuleContext) -> anyhow::Result<ModuleContext> {
        let config: M::Config = ctx.config.section(M::NAME, &[])?;
        Module::register(self, ctx, config).await
    }
}

/// Everything a module is handed at registration time.
///
/// A plain bag of registration inputs. New inputs land here as fields instead
/// of in every module implementation's signature.
pub(crate) struct ModuleContext {
    pub config: AppConfig,
    pub connect_router: ConnectRouter,
}

/// Link-time registry entry. Each module submits one:
///
/// ```
/// inventory::submit! { ModuleRegistration(&MyModule) }
/// ```
pub(crate) struct ModuleRegistration(pub &'static dyn DynModule);
