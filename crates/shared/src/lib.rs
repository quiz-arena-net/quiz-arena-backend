//! What every service in this workspace is built from.
//!
//! Two kinds of sharing live here, and they are deliberately kept apart.
//!
//! [`config`], [`telemetry`], and [`server`] are the plumbing that turns a
//! process into a running service. They know nothing about the domain and
//! every service uses them the same way.
//!
//! [`kernel`] is the shared kernel, the modelling every service is allowed to
//! build on. It is layered like a service is, so a port and its adapters stay
//! as separated there as they are here.
//!
//! A service crate is a binary that composes its own layers and hands the
//! result to [`server::serve`].

pub mod config;
pub mod kernel;
pub mod server;
pub mod telemetry;

/// Identifies the build this binary was produced from.
///
/// CI sets `QUIZ_ARENA_BUILD_VERSION` at compile time to `git describe --tags
/// --match 'v*' --always --dirty`, giving `v0.1.0` on a release tag and
/// `v0.1.0-1-g734713b` off one. The same string belongs on the container image
/// tag and the deployment's labels.
///
/// A local `cargo build` has no CI to set it and reports `dev`.
///
/// Exported as the `service.version` resource attribute, so it lands on every
/// span, metric, and log.
pub const BUILD_VERSION: &str = match option_env!("QUIZ_ARENA_BUILD_VERSION") {
    Some(version) => version,
    None => "dev",
};
