//! Shared kernel: abstractions every module may build on.
//!
//! Layering mirrors the modules themselves so a port and its adapters stay as
//! separated here as they are there.

pub(crate) mod application;
pub(crate) mod infrastructure;
