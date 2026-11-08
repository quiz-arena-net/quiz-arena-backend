use std::collections::BTreeSet;

use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use super::{quiz::QuizId, tag::TagSet, user::UserId};

/// Uniquely identifies a quiz list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizListId(Uuid);

impl QuizListId {
    /// Generates a new identifier.
    pub(crate) fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub(crate) fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub(crate) fn as_uuid(&self) -> Uuid {
        self.0
    }
}

/// The title of a quiz list.
///
/// Guaranteed to be 1 to 100 characters in NFC, with no surrounding whitespace
/// and no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizListTitle(String);

impl QuizListTitle {
    pub(crate) const MAX_LENGTH: usize = 100;

    /// Parses text into a quiz list title.
    ///
    /// Trims surrounding whitespace and normalizes the input text to NFC.
    pub(crate) fn parse(text: impl Into<String>) -> Result<Self, QuizListTitleParseError> {
        let text: String = text.into().trim().nfc().collect();
        if text.is_empty() {
            return Err(QuizListTitleParseError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(QuizListTitleParseError::TooLong { length });
        }
        if text.chars().any(char::is_control) {
            return Err(QuizListTitleParseError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QuizListTitleParseError {
    #[error("quiz list title must not be empty or only whitespace")]
    Blank,
    #[error(
        "quiz list title must be at most {} characters, got {length}",
        QuizListTitle::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("quiz list title must not contain control characters")]
    InvalidCharacter,
}

/// A description of what a quiz list contains.
///
/// Guaranteed to be 1 to 1000 characters in NFC, not only whitespace, with no
/// control characters other than newline and tab.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizListDescription(String);

impl QuizListDescription {
    pub(crate) const MAX_LENGTH: usize = 1000;

    /// Parses text into a quiz list description.
    ///
    /// Normalizes the input text to NFC.
    pub(crate) fn parse(text: impl Into<String>) -> Result<Self, QuizListDescriptionParseError> {
        let text: String = text.into().nfc().collect();
        if text.trim().is_empty() {
            return Err(QuizListDescriptionParseError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(QuizListDescriptionParseError::TooLong { length });
        }
        if text
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
        {
            return Err(QuizListDescriptionParseError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QuizListDescriptionParseError {
    #[error("quiz list description must not be empty or only whitespace")]
    Blank,
    #[error(
        "quiz list description must be at most {} characters, got {length}",
        QuizListDescription::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("quiz list description must not contain control characters other than newline and tab")]
    InvalidCharacter,
}

/// Identifies one version of a quiz list's entries.
///
/// Assigned whenever the quizzes or their order change, so it also serves as a
/// version identifier, distinguishing versions even when the entries are
/// identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct QuizListEntriesId(Uuid);

impl QuizListEntriesId {
    /// Generates a new identifier.
    pub(crate) fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub(crate) fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub(crate) fn as_uuid(&self) -> Uuid {
        self.0
    }
}

/// One version of the quizzes in a quiz list, in order.
///
/// Guaranteed to hold at most 100 quizzes with no quiz twice. May be empty and
/// may refer to quizzes that no longer exist. Two versions are equal when they
/// have the same identifier.
#[derive(Debug, Clone, Eq)]
pub(crate) struct QuizListEntries {
    /// Identifies this version.
    id: QuizListEntriesId,

    /// The quizzes, in order.
    quiz_ids: Vec<QuizId>,
}

impl QuizListEntries {
    pub(crate) const MAX_QUIZZES: usize = 100;

    fn check_quiz_ids(quiz_ids: &[QuizId]) -> Result<(), QuizListEntriesError> {
        let count = quiz_ids.len();
        if count > Self::MAX_QUIZZES {
            return Err(QuizListEntriesError::TooManyQuizzes { count });
        }
        let mut seen = BTreeSet::new();
        for quiz_id in quiz_ids {
            if !seen.insert(quiz_id) {
                return Err(QuizListEntriesError::DuplicateQuiz);
            }
        }
        Ok(())
    }

    /// Creates a new version of the entries with a freshly generated
    /// identifier.
    pub(crate) fn new(quiz_ids: Vec<QuizId>) -> Result<Self, QuizListEntriesError> {
        Self::check_quiz_ids(&quiz_ids)?;
        Ok(Self {
            id: QuizListEntriesId::new(),
            quiz_ids,
        })
    }

    /// Reconstitutes a version of the entries from stored state.
    ///
    /// For repository implementations only. Checks the invariants, which hold
    /// for every version however it was obtained.
    pub(crate) fn reconstitute(
        id: QuizListEntriesId,
        quiz_ids: Vec<QuizId>,
    ) -> Result<Self, QuizListEntriesError> {
        Self::check_quiz_ids(&quiz_ids)?;
        Ok(Self { id, quiz_ids })
    }

    pub(crate) fn id(&self) -> QuizListEntriesId {
        self.id
    }

    pub(crate) fn quiz_ids(&self) -> &[QuizId] {
        &self.quiz_ids
    }
}

impl PartialEq for QuizListEntries {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QuizListEntriesError {
    #[error("a quiz list must not contain the same quiz twice")]
    DuplicateQuiz,
    #[error(
        "a quiz list must have at most {} quizzes, got {count}",
        QuizListEntries::MAX_QUIZZES
    )]
    TooManyQuizzes { count: usize },
}

/// The audience of a quiz list.
///
/// Determines who can find the list, view it and start a session from it.
/// Whether a quiz may be included in the list is determined by its
/// [`InclusionPolicy`](super::quiz::InclusionPolicy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Visibility {
    /// Anyone can reach the list.
    Public,

    /// Anyone with the link can reach the list.
    Unlisted,

    /// Only the owner can reach the list.
    Private,
}

/// An ordered list of quizzes.
///
/// A list refers to quizzes by ID and does not own them. A quiz can appear in
/// multiple lists, including those owned by other users. Deleting a list does
/// not delete its quizzes.
///
/// Whether a quiz can be added is determined by
/// [`Quiz::includable_by`](super::quiz::Quiz::includable_by). The same check
/// runs again when a session builds its lineup. An inclusion that is no longer
/// permitted stays in the list and is skipped.
///
/// The entries are versioned, see [`QuizListEntries`]. The title, description,
/// visibility and tags are not. Edit permissions are checked by
/// [`QuizList::editable_by`]. Two lists are equal when they have the same
/// identifier.
#[derive(Debug, Clone, Eq)]
pub(crate) struct QuizList {
    /// The unique identifier of this list.
    id: QuizListId,

    /// The user who owns this list.
    owner: UserId,

    /// The title of this list.
    title: QuizListTitle,

    /// An optional description of this list.
    description: Option<QuizListDescription>,

    /// The audience of this list.
    visibility: Visibility,

    /// The quizzes in this list at the current version.
    entries: QuizListEntries,

    /// Tags associated with this list.
    tags: TagSet,
}

impl QuizList {
    /// Creates a new quiz list with a freshly generated identifier.
    pub(crate) fn new(
        owner: UserId,
        title: QuizListTitle,
        description: Option<QuizListDescription>,
        visibility: Visibility,
        entries: QuizListEntries,
        tags: TagSet,
    ) -> Self {
        Self {
            id: QuizListId::new(),
            owner,
            title,
            description,
            visibility,
            entries,
            tags,
        }
    }

    /// Reconstitutes a quiz list from stored state.
    ///
    /// For repository implementations only. The aggregate invariants are
    /// carried by the field types, so this cannot fail.
    pub(crate) fn reconstitute(
        id: QuizListId,
        owner: UserId,
        title: QuizListTitle,
        description: Option<QuizListDescription>,
        visibility: Visibility,
        entries: QuizListEntries,
        tags: TagSet,
    ) -> Self {
        Self {
            id,
            owner,
            title,
            description,
            visibility,
            entries,
            tags,
        }
    }

    pub(crate) fn id(&self) -> QuizListId {
        self.id
    }

    pub(crate) fn owner(&self) -> UserId {
        self.owner
    }

    pub(crate) fn title(&self) -> &QuizListTitle {
        &self.title
    }

    pub(crate) fn description(&self) -> Option<&QuizListDescription> {
        self.description.as_ref()
    }

    pub(crate) fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub(crate) fn entries(&self) -> &QuizListEntries {
        &self.entries
    }

    pub(crate) fn tags(&self) -> &TagSet {
        &self.tags
    }

    /// Returns whether `user` may edit this quiz list.
    ///
    /// Only the owner may edit.
    pub(crate) fn editable_by(&self, user: UserId) -> bool {
        user == self.owner
    }

    pub(crate) fn change_title(&mut self, title: QuizListTitle) {
        self.title = title;
    }

    pub(crate) fn change_description(&mut self, description: Option<QuizListDescription>) {
        self.description = description;
    }

    pub(crate) fn change_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    /// Inserts a quiz at `position`, or appends it when `position` is `None`.
    ///
    /// Replaces the entries with a new version.
    pub(crate) fn add_quiz(
        &mut self,
        quiz_id: QuizId,
        position: Option<usize>,
    ) -> Result<(), AddQuizError> {
        let mut quiz_ids = self.entries.quiz_ids().to_vec();
        let position = position.unwrap_or(quiz_ids.len());
        if position > quiz_ids.len() {
            return Err(AddQuizError::PositionOutOfRange { position });
        }
        quiz_ids.insert(position, quiz_id);
        self.entries = QuizListEntries::new(quiz_ids).map_err(|error| match error {
            QuizListEntriesError::DuplicateQuiz => AddQuizError::DuplicateQuiz,
            QuizListEntriesError::TooManyQuizzes { count } => {
                AddQuizError::TooManyQuizzes { count }
            }
        })?;
        Ok(())
    }

    /// Removes a quiz from the list.
    ///
    /// Replaces the entries with a new version.
    pub(crate) fn remove_quiz(&mut self, quiz_id: QuizId) -> Result<(), RemoveQuizError> {
        let mut quiz_ids = self.entries.quiz_ids().to_vec();
        let position = quiz_ids
            .iter()
            .position(|current| *current == quiz_id)
            .ok_or(RemoveQuizError::QuizNotFound)?;
        quiz_ids.remove(position);
        self.entries = QuizListEntries::new(quiz_ids)
            .expect("removing a quiz keeps the entries within their invariants");
        Ok(())
    }

    /// Replaces the order of the quizzes.
    ///
    /// Replaces the entries with a new version. `order` must be a permutation
    /// of the current quizzes.
    pub(crate) fn reorder(&mut self, order: Vec<QuizId>) -> Result<(), ReorderError> {
        let current = self.entries.quiz_ids();
        let is_permutation = order.len() == current.len()
            && order.iter().collect::<BTreeSet<_>>() == current.iter().collect::<BTreeSet<_>>();
        if !is_permutation {
            return Err(ReorderError::NotAPermutation);
        }
        self.entries = QuizListEntries::new(order)
            .expect("a permutation of the entries keeps them within their invariants");
        Ok(())
    }

    pub(crate) fn change_tags(&mut self, tags: TagSet) {
        self.tags = tags;
    }
}

impl PartialEq for QuizList {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum AddQuizError {
    #[error("a quiz list must not contain the same quiz twice")]
    DuplicateQuiz,
    #[error(
        "a quiz list must have at most {} quizzes, got {count}",
        QuizListEntries::MAX_QUIZZES
    )]
    TooManyQuizzes { count: usize },
    #[error("position {position} is beyond the end of the quiz list")]
    PositionOutOfRange { position: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum RemoveQuizError {
    #[error("the quiz is not in the quiz list")]
    QuizNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ReorderError {
    #[error("the new order must contain exactly the quizzes currently in the list")]
    NotAPermutation,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::tag::Tag;

    fn quiz_ids(count: usize) -> Vec<QuizId> {
        (0..count).map(|_| QuizId::new()).collect()
    }

    fn entries(quiz_ids: Vec<QuizId>) -> Result<QuizListEntries, QuizListEntriesError> {
        QuizListEntries::new(quiz_ids)
    }

    fn tags(count: usize) -> TagSet {
        TagSet::new(
            (0..count)
                .map(|index| Tag::parse(format!("tag{index}")).unwrap())
                .collect(),
        )
        .unwrap()
    }

    fn new_quiz_list(quiz_ids: Vec<QuizId>, tags: TagSet) -> QuizList {
        QuizList::new(
            UserId::from_uuid(Uuid::now_v7()),
            QuizListTitle::parse("Capitals").unwrap(),
            None,
            Visibility::Private,
            QuizListEntries::new(quiz_ids).unwrap(),
            tags,
        )
    }

    #[test]
    fn accepts_text_within_bounds() {
        assert!(QuizListTitle::parse("x".repeat(100)).is_ok());
        assert!(QuizListDescription::parse("x".repeat(1000)).is_ok());
    }

    #[test]
    fn rejects_text_over_the_limit() {
        assert_eq!(
            QuizListTitle::parse("x".repeat(101)),
            Err(QuizListTitleParseError::TooLong { length: 101 })
        );
        assert_eq!(
            QuizListDescription::parse("x".repeat(1001)),
            Err(QuizListDescriptionParseError::TooLong { length: 1001 })
        );
    }

    #[test]
    fn rejects_blank_text() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
            assert_eq!(
                QuizListTitle::parse(text),
                Err(QuizListTitleParseError::Blank),
                "{text:?}"
            );
            assert_eq!(
                QuizListDescription::parse(text),
                Err(QuizListDescriptionParseError::Blank),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_control_characters() {
        for text in ["\0", "a\0b", "a\rb", "a\nb", "a\tb", "a\u{7f}b", "a\u{85}b"] {
            assert_eq!(
                QuizListTitle::parse(text),
                Err(QuizListTitleParseError::InvalidCharacter),
                "{text:?}"
            );
        }
        for text in ["\0", "a\0b", "a\rb", "a\u{7f}b", "a\u{85}b"] {
            assert_eq!(
                QuizListDescription::parse(text),
                Err(QuizListDescriptionParseError::InvalidCharacter),
                "{text:?}"
            );
        }
    }

    #[test]
    fn keeps_newlines_tabs_and_surrounding_whitespace_in_multi_line_text() {
        let text = "  first line\n\tsecond line\n";
        assert_eq!(QuizListDescription::parse(text).unwrap().as_str(), text);
    }

    #[test]
    fn trims_surrounding_whitespace() {
        for text in [" Tokyo ", "\tTokyo\n", "\u{3000}Tokyo"] {
            assert_eq!(
                QuizListTitle::parse(text).unwrap().as_str(),
                "Tokyo",
                "{text:?}"
            );
        }
    }

    #[test]
    fn normalizes_to_nfc() {
        assert_eq!(QuizListTitle::parse("a\u{301}").unwrap().as_str(), "\u{e1}");
        assert_eq!(QuizListTitle::parse("か\u{3099}").unwrap().as_str(), "が");
        assert_eq!(
            QuizListDescription::parse("a\u{301}").unwrap().as_str(),
            "\u{e1}"
        );
        assert_eq!(
            QuizListDescription::parse("か\u{3099}").unwrap().as_str(),
            "が"
        );
    }

    #[test]
    fn creates_empty_entries() {
        let entries = entries(Vec::new()).unwrap();

        assert!(entries.quiz_ids().is_empty());
    }

    #[test]
    fn creates_entries_at_the_limit() {
        let entries = entries(quiz_ids(100)).unwrap();

        assert_eq!(entries.quiz_ids().len(), 100);
    }

    #[test]
    fn rejects_duplicate_quiz() {
        let quiz_id = QuizId::new();

        assert_eq!(
            entries(vec![quiz_id, QuizId::new(), quiz_id]),
            Err(QuizListEntriesError::DuplicateQuiz)
        );
    }

    #[test]
    fn rejects_more_than_a_hundred_quizzes() {
        assert_eq!(
            entries(quiz_ids(101)),
            Err(QuizListEntriesError::TooManyQuizzes { count: 101 })
        );
    }

    #[test]
    fn entries_versions_are_equal_by_identifier() {
        let ids = quiz_ids(2);
        let first = entries(ids.clone()).unwrap();
        let second = entries(ids).unwrap();

        assert_eq!(first, first.clone());
        assert_ne!(first, second);
    }

    #[test]
    fn quiz_lists_are_equal_by_identifier() {
        let quiz_list = new_quiz_list(Vec::new(), tags(0));
        let mut edited = quiz_list.clone();
        edited.change_title(QuizListTitle::parse("Renamed").unwrap());

        assert_eq!(quiz_list, edited);
        assert_ne!(quiz_list, new_quiz_list(Vec::new(), tags(0)));
    }

    #[test]
    fn only_the_owner_may_edit() {
        let quiz_list = new_quiz_list(Vec::new(), tags(0));

        assert!(quiz_list.editable_by(quiz_list.owner()));
        assert!(!quiz_list.editable_by(UserId::from_uuid(Uuid::now_v7())));
    }

    #[test]
    fn changes_visibility() {
        let mut quiz_list = new_quiz_list(Vec::new(), tags(0));

        quiz_list.change_visibility(Visibility::Public);

        assert_eq!(quiz_list.visibility(), Visibility::Public);
    }

    #[test]
    fn appends_a_quiz() {
        let ids = quiz_ids(2);
        let mut quiz_list = new_quiz_list(ids.clone(), tags(0));
        let first_version = quiz_list.entries().id();
        let quiz_id = QuizId::new();

        quiz_list.add_quiz(quiz_id, None).unwrap();

        assert_eq!(quiz_list.entries().quiz_ids(), [ids[0], ids[1], quiz_id]);
        assert_ne!(quiz_list.entries().id(), first_version);
    }

    #[test]
    fn inserts_a_quiz_at_a_position() {
        let ids = quiz_ids(2);
        let mut quiz_list = new_quiz_list(ids.clone(), tags(0));
        let quiz_id = QuizId::new();

        quiz_list.add_quiz(quiz_id, Some(1)).unwrap();

        assert_eq!(quiz_list.entries().quiz_ids(), [ids[0], quiz_id, ids[1]]);
    }

    #[test]
    fn inserts_a_quiz_at_the_end_position() {
        let ids = quiz_ids(2);
        let mut quiz_list = new_quiz_list(ids.clone(), tags(0));
        let quiz_id = QuizId::new();

        quiz_list.add_quiz(quiz_id, Some(2)).unwrap();

        assert_eq!(quiz_list.entries().quiz_ids(), [ids[0], ids[1], quiz_id]);
    }

    #[test]
    fn rejects_a_position_beyond_the_end() {
        let mut quiz_list = new_quiz_list(quiz_ids(2), tags(0));
        let before = quiz_list.clone();

        assert_eq!(
            quiz_list.add_quiz(QuizId::new(), Some(3)),
            Err(AddQuizError::PositionOutOfRange { position: 3 })
        );
        assert_eq!(quiz_list.entries().id(), before.entries().id());
    }

    #[test]
    fn rejects_adding_a_quiz_twice() {
        let ids = quiz_ids(2);
        let mut quiz_list = new_quiz_list(ids.clone(), tags(0));
        let before = quiz_list.clone();

        assert_eq!(
            quiz_list.add_quiz(ids[0], None),
            Err(AddQuizError::DuplicateQuiz)
        );
        assert_eq!(quiz_list.entries().id(), before.entries().id());
    }

    #[test]
    fn rejects_adding_beyond_the_limit() {
        let mut quiz_list = new_quiz_list(quiz_ids(100), tags(0));
        let before = quiz_list.clone();

        assert_eq!(
            quiz_list.add_quiz(QuizId::new(), None),
            Err(AddQuizError::TooManyQuizzes { count: 101 })
        );
        assert_eq!(quiz_list.entries().id(), before.entries().id());
    }

    #[test]
    fn removes_a_quiz() {
        let ids = quiz_ids(3);
        let mut quiz_list = new_quiz_list(ids.clone(), tags(0));
        let first_version = quiz_list.entries().id();

        quiz_list.remove_quiz(ids[1]).unwrap();

        assert_eq!(quiz_list.entries().quiz_ids(), [ids[0], ids[2]]);
        assert_ne!(quiz_list.entries().id(), first_version);
    }

    #[test]
    fn rejects_removing_an_absent_quiz() {
        let mut quiz_list = new_quiz_list(quiz_ids(2), tags(0));
        let before = quiz_list.clone();

        assert_eq!(
            quiz_list.remove_quiz(QuizId::new()),
            Err(RemoveQuizError::QuizNotFound)
        );
        assert_eq!(quiz_list.entries().id(), before.entries().id());
    }

    #[test]
    fn reorders_the_quizzes() {
        let ids = quiz_ids(3);
        let mut quiz_list = new_quiz_list(ids.clone(), tags(0));
        let first_version = quiz_list.entries().id();

        quiz_list.reorder(vec![ids[2], ids[0], ids[1]]).unwrap();

        assert_eq!(quiz_list.entries().quiz_ids(), [ids[2], ids[0], ids[1]]);
        assert_ne!(quiz_list.entries().id(), first_version);
    }

    #[test]
    fn rejects_an_order_that_is_not_a_permutation() {
        let ids = quiz_ids(3);
        let mut quiz_list = new_quiz_list(ids.clone(), tags(0));
        let before = quiz_list.clone();

        for order in [
            vec![ids[0], ids[1]],
            vec![ids[0], ids[1], ids[2], QuizId::new()],
            vec![ids[0], ids[1], QuizId::new()],
            vec![ids[0], ids[0], ids[1]],
        ] {
            assert_eq!(quiz_list.reorder(order), Err(ReorderError::NotAPermutation));
        }
        assert_eq!(quiz_list.entries().id(), before.entries().id());
    }
}
