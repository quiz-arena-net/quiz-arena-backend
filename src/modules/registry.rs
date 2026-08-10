//! Module registry.
//!
//! Modules register themselves: implement [`Module`](crate::Module) and submit
//! a [`ModuleRegistration`] with `inventory::submit!`.

use crate::{DynModule, ModuleContext, ModuleRegistration};

fn enabled() -> impl Iterator<Item = &'static dyn DynModule> {
    inventory::iter::<ModuleRegistration>
        .into_iter()
        .map(|registration| registration.0)
}

/// Names of every enabled module.
/// Also their config section names by construction.
pub(crate) fn names() -> Vec<&'static str> {
    enabled().map(|module| module.name()).collect()
}

/// Full service names of every enabled module, for health reporting.
pub(crate) fn service_names() -> Vec<&'static str> {
    enabled()
        .flat_map(|module| module.service_names().iter().copied())
        .collect()
}

/// Registers every enabled module's services on the context.
pub(crate) async fn register(mut ctx: ModuleContext) -> anyhow::Result<ModuleContext> {
    for module in enabled() {
        ctx = module.register(ctx).await?;
    }
    Ok(ctx)
}

inventory::collect!(ModuleRegistration);
