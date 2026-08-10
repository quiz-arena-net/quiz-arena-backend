mod registry;

#[cfg(feature = "greet")]
pub(crate) mod greet;

pub(crate) use registry::{names, register, service_names};
