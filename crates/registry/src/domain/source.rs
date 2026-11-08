use chrono::{DateTime, Utc};
use unicode_normalization::UnicodeNormalization;

use super::{user::UserId, web_url::WebUrl};

/// A credit line naming where a quiz came from.
///
/// Guaranteed to be 1 to 200 characters in NFC, with no surrounding whitespace
/// and no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct SourceCredit(String);

impl SourceCredit {
    pub(crate) const MAX_LENGTH: usize = 200;

    /// Parses text into a source credit.
    ///
    /// Trims surrounding whitespace and normalizes the input text to NFC.
    pub(crate) fn parse(text: impl Into<String>) -> Result<Self, SourceCreditParseError> {
        let text: String = text.into().trim().nfc().collect();
        if text.is_empty() {
            return Err(SourceCreditParseError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(SourceCreditParseError::TooLong { length });
        }
        if text.chars().any(char::is_control) {
            return Err(SourceCreditParseError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum SourceCreditParseError {
    #[error("source credit must not be empty or only whitespace")]
    Blank,
    #[error(
        "source credit must be at most {} characters, got {length}",
        SourceCredit::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("source credit must not contain control characters")]
    InvalidCharacter,
}

/// Where a quiz came from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Source {
    /// Written by a user of this service.
    User {
        /// The user who wrote the quiz.
        author: UserId,
    },

    /// Imported by an operator from outside this service.
    External {
        /// Who to credit.
        credit: SourceCredit,

        /// Where the quiz was taken from.
        url: Option<WebUrl>,

        /// When the quiz and its usage terms were retrieved.
        retrieved_at: DateTime<Utc>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_credit_within_bounds() {
        assert!(SourceCredit::parse("x".repeat(200)).is_ok());
    }

    #[test]
    fn rejects_credit_over_the_limit() {
        assert_eq!(
            SourceCredit::parse("x".repeat(201)),
            Err(SourceCreditParseError::TooLong { length: 201 })
        );
    }

    #[test]
    fn rejects_blank_credit() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
            assert_eq!(
                SourceCredit::parse(text),
                Err(SourceCreditParseError::Blank),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_control_characters_in_credit() {
        for text in ["\0", "a\0b", "a\rb", "a\nb", "a\tb", "a\u{7f}b", "a\u{85}b"] {
            assert_eq!(
                SourceCredit::parse(text),
                Err(SourceCreditParseError::InvalidCharacter),
                "{text:?}"
            );
        }
    }

    #[test]
    fn trims_and_normalizes_credit() {
        assert_eq!(
            SourceCredit::parse(" Quiz Works\n").unwrap().as_str(),
            "Quiz Works"
        );
        assert_eq!(SourceCredit::parse("か\u{3099}").unwrap().as_str(), "が");
    }
}
