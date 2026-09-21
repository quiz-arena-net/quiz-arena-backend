use facet::Facet;

pub(crate) const REGISTRY_SECTION_NAME: &str = "registry";

/// The `[registry]` config section.
#[derive(Debug, Facet)]
#[facet(deny_unknown_fields)]
pub(super) struct RegistryConfig {}
