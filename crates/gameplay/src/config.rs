use facet::Facet;

pub(crate) const GAMEPLAY_SECTION_NAME: &str = "gameplay";

/// The `[gameplay]` config section.
#[derive(Debug, Facet)]
#[facet(deny_unknown_fields)]
pub(super) struct GameplayConfig {}
