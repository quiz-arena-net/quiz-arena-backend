use facet::Facet;

pub(crate) const GREET_SECTION_NAME: &str = "greet";

/// The `[greet]` config section.
#[derive(Debug, Facet)]
#[facet(deny_unknown_fields)]
pub(super) struct GreetConfig {
    /// Connection URL for the greeting store.
    #[facet(default = "sqlite::memory:".to_owned())]
    pub database_url: String,
}
