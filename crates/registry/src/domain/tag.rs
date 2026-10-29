/// A tag used to classify or discover quizzes and quiz lists.
///
/// Guaranteed to be 1 to 32 characters, not only whitespace, and free of NUL.
/// Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Tag(String);

impl Tag {
    pub(crate) const MAX_LENGTH: usize = 32;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, TagError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(TagError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(TagError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(TagError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TagError {
    #[error("tag must not be empty or only whitespace")]
    Blank,
    #[error("tag must be at most {} characters, got {length}", Tag::MAX_LENGTH)]
    TooLong { length: usize },
    #[error("tag must not contain NUL")]
    InvalidCharacter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_text_within_bounds() {
        assert!(Tag::new("x".repeat(32)).is_ok());
    }

    #[test]
    fn rejects_text_over_the_limit() {
        assert_eq!(
            Tag::new("x".repeat(33)),
            Err(TagError::TooLong { length: 33 })
        );
    }

    #[test]
    fn counts_length_in_characters_not_bytes() {
        assert!(Tag::new("あ".repeat(32)).is_ok());
        assert_eq!(
            Tag::new("あ".repeat(33)),
            Err(TagError::TooLong { length: 33 })
        );
    }

    #[test]
    fn rejects_blank_text() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
            assert_eq!(Tag::new(text), Err(TagError::Blank), "{text:?}");
        }
    }

    #[test]
    fn rejects_text_containing_nul() {
        for text in ["\0", "a\0", "\0a", "a\0b"] {
            assert_eq!(Tag::new(text), Err(TagError::InvalidCharacter), "{text:?}");
        }
    }

    #[test]
    fn preserves_accepted_text_exactly() {
        for text in [" padded ", "Tokyo", "東京", "a\u{301}"] {
            assert_eq!(Tag::new(text).unwrap().as_str(), text);
        }
    }
}
