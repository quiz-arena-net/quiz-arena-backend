use unicode_normalization::UnicodeNormalization;

use super::web_url::WebUrl;

/// Usage terms written out by a rights holder.
///
/// Guaranteed to be 1 to 1000 characters in NFC, not only whitespace, with no
/// control characters other than newline and tab.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct TermsDescription(String);

impl TermsDescription {
    pub(crate) const MAX_LENGTH: usize = 1000;

    /// Parses text into a terms description.
    ///
    /// Normalizes the input text to NFC.
    pub(crate) fn parse(text: impl Into<String>) -> Result<Self, TermsDescriptionParseError> {
        let text: String = text.into().nfc().collect();
        if text.trim().is_empty() {
            return Err(TermsDescriptionParseError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(TermsDescriptionParseError::TooLong { length });
        }
        if text
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
        {
            return Err(TermsDescriptionParseError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TermsDescriptionParseError {
    #[error("terms description must not be empty or only whitespace")]
    Blank,
    #[error(
        "terms description must be at most {} characters, got {length}",
        TermsDescription::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("terms description must not contain control characters other than newline and tab")]
    InvalidCharacter,
}

/// The conditions a Creative Commons license imposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum CreativeCommonsKind {
    /// CC BY. Attribution.
    By,
    /// CC BY-SA. Attribution, share alike.
    BySa,
    /// CC BY-NC. Attribution, non-commercial.
    ByNc,
    /// CC BY-NC-SA. Attribution, non-commercial, share alike.
    ByNcSa,
    /// CC BY-ND. Attribution, no derivatives.
    ByNd,
    /// CC BY-NC-ND. Attribution, non-commercial, no derivatives.
    ByNcNd,
}

/// The version of a Creative Commons license.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum CreativeCommonsVersion {
    V4_0,
}

/// A Creative Commons license.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CreativeCommons {
    /// The conditions the license imposes.
    kind: CreativeCommonsKind,

    /// The version of the license.
    version: CreativeCommonsVersion,
}

impl CreativeCommons {
    pub(crate) fn new(kind: CreativeCommonsKind, version: CreativeCommonsVersion) -> Self {
        Self { kind, version }
    }

    pub(crate) fn kind(&self) -> CreativeCommonsKind {
        self.kind
    }

    pub(crate) fn version(&self) -> CreativeCommonsVersion {
        self.version
    }
}

/// Usage terms written by the rights holder outside a standard license.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CustomTerms {
    /// What the rights holder allows.
    description: TermsDescription,

    /// Where the terms are published.
    url: Option<WebUrl>,

    /// Whether reuse requires crediting the rights holder.
    requires_attribution: bool,
}

impl CustomTerms {
    pub(crate) fn new(
        description: TermsDescription,
        url: Option<WebUrl>,
        requires_attribution: bool,
    ) -> Self {
        Self {
            description,
            url,
            requires_attribution,
        }
    }

    pub(crate) fn description(&self) -> &TermsDescription {
        &self.description
    }

    pub(crate) fn url(&self) -> Option<&WebUrl> {
        self.url.as_ref()
    }

    pub(crate) fn requires_attribution(&self) -> bool {
        self.requires_attribution
    }
}

/// The terms under which a quiz may be used.
///
/// Shown to users so they know what they may do with the quiz outside this
/// service. Use within this service is covered by the terms of service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum UsageTerms {
    /// No reuse is allowed. This is the default under copyright law.
    AllRightsReserved,

    /// Reuse is allowed under a Creative Commons license.
    CreativeCommons(CreativeCommons),

    /// Reuse is unrestricted. Covers public domain works and CC0 dedications.
    PublicDomain,

    /// Reuse is allowed under terms the rights holder wrote out.
    Custom(CustomTerms),
}

impl UsageTerms {
    /// Returns whether reusing the quiz requires crediting the rights holder.
    ///
    /// This is about reuse by users. The credit of an external source is shown
    /// regardless.
    pub(crate) fn requires_attribution(&self) -> bool {
        match self {
            Self::AllRightsReserved | Self::PublicDomain => false,
            Self::CreativeCommons(_) => true,
            Self::Custom(terms) => terms.requires_attribution(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_description_within_bounds() {
        assert!(TermsDescription::parse("x".repeat(1000)).is_ok());
    }

    #[test]
    fn rejects_description_over_the_limit() {
        assert_eq!(
            TermsDescription::parse("x".repeat(1001)),
            Err(TermsDescriptionParseError::TooLong { length: 1001 })
        );
    }

    #[test]
    fn rejects_blank_description() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
            assert_eq!(
                TermsDescription::parse(text),
                Err(TermsDescriptionParseError::Blank),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_control_characters_other_than_newline_and_tab_in_description() {
        for text in ["\0", "a\0b", "a\rb", "a\u{7f}b", "a\u{85}b"] {
            assert_eq!(
                TermsDescription::parse(text),
                Err(TermsDescriptionParseError::InvalidCharacter),
                "{text:?}"
            );
        }
    }

    #[test]
    fn keeps_newlines_tabs_and_surrounding_whitespace_in_description() {
        let text = "  free to reuse\n\tcredit appreciated\n";
        assert_eq!(TermsDescription::parse(text).unwrap().as_str(), text);
    }

    #[test]
    fn normalizes_description_to_nfc() {
        assert_eq!(
            TermsDescription::parse("a\u{301}").unwrap().as_str(),
            "\u{e1}"
        );
    }

    #[test]
    fn attribution_follows_the_terms() {
        assert!(!UsageTerms::AllRightsReserved.requires_attribution());
        assert!(!UsageTerms::PublicDomain.requires_attribution());
        assert!(
            UsageTerms::CreativeCommons(CreativeCommons::new(
                CreativeCommonsKind::By,
                CreativeCommonsVersion::V4_0
            ))
            .requires_attribution()
        );
        let description = TermsDescription::parse("Free to reuse.").unwrap();
        assert!(
            !UsageTerms::Custom(CustomTerms::new(description.clone(), None, false))
                .requires_attribution()
        );
        assert!(
            UsageTerms::Custom(CustomTerms::new(description, None, true)).requires_attribution()
        );
    }
}
