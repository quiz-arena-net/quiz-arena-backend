//! Shared kernel
//!
//! Layered the way a service is, so a port and its adapters stay as separated
//! here as they are there.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
