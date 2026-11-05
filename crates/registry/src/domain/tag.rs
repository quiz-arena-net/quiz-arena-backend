use unicode_normalization::UnicodeNormalization;

/// A tag used to classify or discover quizzes and quiz lists.
///
/// Guaranteed to be 1 to 32 characters in NFC, with no surrounding whitespace
/// and no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Tag(String);

impl Tag {
    pub(crate) const MAX_LENGTH: usize = 32;

    /// Parses text into a tag.
    ///
    /// Trims surrounding whitespace and normalizes the input text to NFC.
    pub(crate) fn parse(text: impl Into<String>) -> Result<Self, TagParseError> {
        let text: String = text.into().trim().nfc().collect();
        if text.is_empty() {
            return Err(TagParseError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(TagParseError::TooLong { length });
        }
        if text.chars().any(char::is_control) {
            return Err(TagParseError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TagParseError {
    #[error("tag must not be empty or only whitespace")]
    Blank,
    #[error("tag must be at most {} characters, got {length}", Tag::MAX_LENGTH)]
    TooLong { length: usize },
    #[error("tag must not contain control characters")]
    InvalidCharacter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_text_within_bounds() {
        assert!(Tag::parse("x".repeat(32)).is_ok());
    }

    #[test]
    fn rejects_text_over_the_limit() {
        assert_eq!(
            Tag::parse("x".repeat(33)),
            Err(TagParseError::TooLong { length: 33 })
        );
    }

    #[test]
    fn counts_length_in_characters_not_bytes() {
        assert!(Tag::parse("あ".repeat(32)).is_ok());
        assert_eq!(
            Tag::parse("あ".repeat(33)),
            Err(TagParseError::TooLong { length: 33 })
        );
    }

    #[test]
    fn rejects_blank_text() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
            assert_eq!(Tag::parse(text), Err(TagParseError::Blank), "{text:?}");
        }
    }

    #[test]
    fn rejects_control_characters() {
        for text in ["\0", "a\0b", "a\rb", "a\nb", "a\tb", "a\u{7f}b", "a\u{85}b"] {
            assert_eq!(
                Tag::parse(text),
                Err(TagParseError::InvalidCharacter),
                "{text:?}"
            );
        }
    }

    #[test]
    fn trims_surrounding_whitespace() {
        for text in [" Tokyo ", "\tTokyo\n", "\u{3000}Tokyo"] {
            assert_eq!(Tag::parse(text).unwrap().as_str(), "Tokyo", "{text:?}");
        }
    }

    #[test]
    fn normalizes_to_nfc() {
        assert_eq!(Tag::parse("a\u{301}").unwrap().as_str(), "\u{e1}");
        assert_eq!(Tag::parse("か\u{3099}").unwrap().as_str(), "が");
    }
}
