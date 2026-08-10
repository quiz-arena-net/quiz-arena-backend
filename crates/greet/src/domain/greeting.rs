use std::fmt;

use async_trait::async_trait;

use quiz_arena_shared::kernel::domain::RepositoryError;

/// Name of a client sending a greeting.
///
/// Guaranteed to be between 1 and 32 characters long. Lengths are counted in
/// Unicode characters, matching the semantics of protovalidate's
/// `string.min_len` / `string.max_len` rules on the proto field.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SenderName(String);

impl SenderName {
    pub(crate) const MAX_LENGTH: usize = 32;

    pub(crate) fn new(name: impl Into<String>) -> Result<Self, SenderNameError> {
        let name = name.into();
        if name.is_empty() {
            return Err(SenderNameError::Empty);
        }
        let length = name.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(SenderNameError::TooLong { length });
        }
        Ok(Self(name))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SenderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum SenderNameError {
    #[error("sender name must not be empty")]
    Empty,
    #[error(
        "sender name must be at most {} characters, got {length}",
        SenderName::MAX_LENGTH
    )]
    TooLong { length: usize },
}

/// A sender's greeting history: who greeted and how many times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Greeting {
    sender: SenderName,
    times_greeted: u32,
}

impl Greeting {
    /// A sender's first greeting.
    pub(crate) fn first(sender: SenderName) -> Self {
        Self {
            sender,
            times_greeted: 1,
        }
    }

    /// Rehydrates a greeting history from persisted state.
    ///
    /// For repository implementations only. Trusts the stored count, which
    /// this aggregate produced in the first place.
    pub(crate) fn from_persistence(sender: SenderName, times_greeted: u32) -> Self {
        Self {
            sender,
            times_greeted,
        }
    }

    /// Records another greeting from the same sender.
    pub(crate) fn record_another(&mut self) {
        self.times_greeted = self.times_greeted.saturating_add(1);
    }

    pub(crate) fn sender(&self) -> &SenderName {
        &self.sender
    }

    pub(crate) fn times_greeted(&self) -> u32 {
        self.times_greeted
    }

    /// Whether the sender had already greeted before the latest greeting.
    pub(crate) fn is_returning(&self) -> bool {
        self.times_greeted > 1
    }
}

/// Port for persisting greeting histories.
///
/// Failures cross the boundary as the shared [`RepositoryError`]. What a
/// rollback means for greetings (rerun the unit of work) is the use case's
/// decision, not the repository's.
#[async_trait]
pub(crate) trait GreetingRepository: Send + Sync {
    async fn find_by_sender(
        &self,
        sender: &SenderName,
    ) -> Result<Option<Greeting>, RepositoryError>;

    async fn add(&self, greeting: &Greeting) -> Result<(), RepositoryError>;

    async fn update(&self, greeting: &Greeting) -> Result<(), RepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_names_within_bounds() {
        let name = SenderName::new("alice").unwrap();
        assert_eq!(name.as_str(), "alice");

        assert!(SenderName::new("a").is_ok());
        assert!(SenderName::new("x".repeat(32)).is_ok());
    }

    #[test]
    fn rejects_empty_name() {
        assert_eq!(SenderName::new(""), Err(SenderNameError::Empty));
    }

    #[test]
    fn rejects_name_longer_than_32_characters() {
        assert_eq!(
            SenderName::new("x".repeat(33)),
            Err(SenderNameError::TooLong { length: 33 })
        );
    }

    #[test]
    fn counts_length_in_characters_not_bytes() {
        // 32 characters but 96 bytes in UTF-8, which is valid under
        // protovalidate semantics.
        assert!(SenderName::new("あ".repeat(32)).is_ok());
    }

    #[test]
    fn first_greeting_is_not_returning() {
        let greeting = Greeting::first(SenderName::new("alice").unwrap());

        assert_eq!(greeting.times_greeted(), 1);
        assert!(!greeting.is_returning());
    }

    #[test]
    fn repeated_greeting_is_returning() {
        let mut greeting = Greeting::first(SenderName::new("alice").unwrap());
        greeting.record_another();

        assert_eq!(greeting.times_greeted(), 2);
        assert!(greeting.is_returning());
    }
}
