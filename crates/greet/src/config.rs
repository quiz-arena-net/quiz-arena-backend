use serde::{Deserialize, Serialize};

pub(crate) const GREET_SECTION_NAME: &str = "greet";

/// The `[greet]` config section.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GreetConfig {
    /// Connection URL for the greeting store.
    pub database_url: String,
}

impl Default for GreetConfig {
    fn default() -> Self {
        Self {
            database_url: "sqlite::memory:".to_owned(),
        }
    }
}
