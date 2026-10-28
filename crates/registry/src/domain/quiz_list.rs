use std::collections::BTreeSet;

use uuid::Uuid;

use super::{quiz::QuizId, tag::Tag, user::UserId};

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
/// Guaranteed to be 1 to 100 characters, not only whitespace, and free of NUL.
/// Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizListTitle(String);

impl QuizListTitle {
    pub(crate) const MAX_LENGTH: usize = 100;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, QuizListTitleError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(QuizListTitleError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(QuizListTitleError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(QuizListTitleError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QuizListTitleError {
    #[error("quiz list title must not be empty or only whitespace")]
    Blank,
    #[error(
        "quiz list title must be at most {} characters, got {length}",
        QuizListTitle::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("quiz list title must not contain NUL")]
    InvalidCharacter,
}

/// A description of what a quiz list contains.
///
/// Guaranteed to be 1 to 1000 characters, not only whitespace, and free of NUL.
/// Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizListDescription(String);

impl QuizListDescription {
    pub(crate) const MAX_LENGTH: usize = 1000;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, QuizListDescriptionError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(QuizListDescriptionError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(QuizListDescriptionError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(QuizListDescriptionError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QuizListDescriptionError {
    #[error("quiz list description must not be empty or only whitespace")]
    Blank,
    #[error(
        "quiz list description must be at most {} characters, got {length}",
        QuizListDescription::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("quiz list description must not contain NUL")]
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
/// may refer to quizzes that no longer exist.
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Rehydrates a version of the entries from persisted state.
    ///
    /// For repository implementations only. Checks the invariants, which hold
    /// for every version however it was obtained.
    pub(crate) fn from_persistence(
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
/// visibility and tags are not. A list can carry up to 10 tags. Edit
/// permissions are checked by [`QuizList::editable_by`].
#[derive(Debug, Clone, PartialEq, Eq)]
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
    tags: BTreeSet<Tag>,
}

impl QuizList {
    pub(crate) const MAX_TAGS: usize = 10;

    fn check_tags(tags: &BTreeSet<Tag>) -> Result<(), QuizListError> {
        let count = tags.len();
        if count > Self::MAX_TAGS {
            return Err(QuizListError::TooManyTags { count });
        }
        Ok(())
    }

    /// Creates a new quiz list with a freshly generated identifier and the
    /// first version of its entries.
    pub(crate) fn new(
        owner: UserId,
        title: QuizListTitle,
        description: Option<QuizListDescription>,
        visibility: Visibility,
        quiz_ids: Vec<QuizId>,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, QuizListError> {
        let entries = QuizListEntries::new(quiz_ids).map_err(|error| match error {
            QuizListEntriesError::DuplicateQuiz => QuizListError::DuplicateQuiz,
            QuizListEntriesError::TooManyQuizzes { count } => {
                QuizListError::TooManyQuizzes { count }
            }
        })?;
        Self::check_tags(&tags)?;
        Ok(Self {
            id: QuizListId::new(),
            owner,
            title,
            description,
            visibility,
            entries,
            tags,
        })
    }

    /// Rehydrates a quiz list from persisted state.
    ///
    /// For repository implementations only. Checks the aggregate invariants,
    /// which hold for every quiz list however it was obtained. Restrictions
    /// that apply only when creating a quiz list are not rechecked here.
    pub(crate) fn from_persistence(
        id: QuizListId,
        owner: UserId,
        title: QuizListTitle,
        description: Option<QuizListDescription>,
        visibility: Visibility,
        entries: QuizListEntries,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, QuizListError> {
        Self::check_tags(&tags)?;
        Ok(Self {
            id,
            owner,
            title,
            description,
            visibility,
            entries,
            tags,
        })
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

    pub(crate) fn tags(&self) -> &BTreeSet<Tag> {
        &self.tags
    }

    /// Returns whether `user` may edit this quiz list.
    ///
    /// Only the owner may edit.
    pub(crate) fn editable_by(&self, user: UserId) -> bool {
        let _ = user;
        todo!()
    }

    pub(crate) fn change_title(&mut self, title: QuizListTitle) {
        self.title = title;
    }

    pub(crate) fn change_description(&mut self, description: Option<QuizListDescription>) {
        self.description = description;
    }

    pub(crate) fn change_visibility(&mut self, visibility: Visibility) {
        let _ = visibility;
        todo!()
    }

    /// Inserts a quiz at `position`, or appends it when `position` is `None`.
    ///
    /// Replaces the entries with a new version.
    pub(crate) fn add_quiz(
        &mut self,
        quiz_id: QuizId,
        position: Option<usize>,
    ) -> Result<(), AddQuizError> {
        let _ = (quiz_id, position);
        todo!()
    }

    /// Removes a quiz from the list.
    ///
    /// Replaces the entries with a new version.
    pub(crate) fn remove_quiz(&mut self, quiz_id: QuizId) -> Result<(), RemoveQuizError> {
        let _ = quiz_id;
        todo!()
    }

    /// Replaces the order of the quizzes.
    ///
    /// Replaces the entries with a new version. `order` must be a permutation
    /// of the current quizzes.
    pub(crate) fn reorder(&mut self, order: Vec<QuizId>) -> Result<(), ReorderError> {
        let _ = order;
        todo!()
    }

    /// Replaces the tags.
    pub(crate) fn change_tags(&mut self, tags: BTreeSet<Tag>) -> Result<(), ChangeTagsError> {
        let count = tags.len();
        if count > Self::MAX_TAGS {
            return Err(ChangeTagsError::TooManyTags { count });
        }
        self.tags = tags;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QuizListError {
    #[error("a quiz list must not contain the same quiz twice")]
    DuplicateQuiz,
    #[error(
        "a quiz list must have at most {} quizzes, got {count}",
        QuizListEntries::MAX_QUIZZES
    )]
    TooManyQuizzes { count: usize },
    #[error(
        "a quiz list must have at most {} tags, got {count}",
        QuizList::MAX_TAGS
    )]
    TooManyTags { count: usize },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ChangeTagsError {
    #[error(
        "a quiz list must have at most {} tags, got {count}",
        QuizList::MAX_TAGS
    )]
    TooManyTags { count: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiz_ids(count: usize) -> Vec<QuizId> {
        (0..count).map(|_| QuizId::new()).collect()
    }

    fn entries(quiz_ids: Vec<QuizId>) -> Result<QuizListEntries, QuizListEntriesError> {
        QuizListEntries::new(quiz_ids)
    }

    fn tags(count: usize) -> BTreeSet<Tag> {
        (0..count)
            .map(|index| Tag::new(format!("tag{index}")).unwrap())
            .collect()
    }

    fn new_quiz_list(
        quiz_ids: Vec<QuizId>,
        tags: BTreeSet<Tag>,
    ) -> Result<QuizList, QuizListError> {
        QuizList::new(
            UserId::from_uuid(Uuid::now_v7()),
            QuizListTitle::new("Capitals").unwrap(),
            None,
            Visibility::Private,
            quiz_ids,
            tags,
        )
    }

    #[test]
    fn accepts_text_within_bounds() {
        assert!(QuizListTitle::new("x".repeat(100)).is_ok());
        assert!(QuizListDescription::new("x".repeat(1000)).is_ok());
    }

    #[test]
    fn rejects_text_over_the_limit() {
        assert_eq!(
            QuizListTitle::new("x".repeat(101)),
            Err(QuizListTitleError::TooLong { length: 101 })
        );
        assert_eq!(
            QuizListDescription::new("x".repeat(1001)),
            Err(QuizListDescriptionError::TooLong { length: 1001 })
        );
    }

    #[test]
    fn rejects_blank_text() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
            assert_eq!(
                QuizListTitle::new(text),
                Err(QuizListTitleError::Blank),
                "{text:?}"
            );
            assert_eq!(
                QuizListDescription::new(text),
                Err(QuizListDescriptionError::Blank),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_text_containing_nul() {
        for text in ["\0", "a\0", "\0a", "a\0b"] {
            assert_eq!(
                QuizListTitle::new(text),
                Err(QuizListTitleError::InvalidCharacter),
                "{text:?}"
            );
            assert_eq!(
                QuizListDescription::new(text),
                Err(QuizListDescriptionError::InvalidCharacter),
                "{text:?}"
            );
        }
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
    fn creates_quiz_list_at_the_tag_limit() {
        let quiz_list = new_quiz_list(Vec::new(), tags(10)).unwrap();

        assert_eq!(quiz_list.tags().len(), 10);
    }

    #[test]
    fn rejects_more_than_ten_tags() {
        assert_eq!(
            new_quiz_list(Vec::new(), tags(11)),
            Err(QuizListError::TooManyTags { count: 11 })
        );
    }

    #[test]
    fn rehydration_checks_invariants() {
        let rehydrate = |tags| {
            QuizList::from_persistence(
                QuizListId::new(),
                UserId::from_uuid(Uuid::now_v7()),
                QuizListTitle::new("Capitals").unwrap(),
                None,
                Visibility::Private,
                entries(Vec::new()).unwrap(),
                tags,
            )
        };

        assert!(rehydrate(tags(10)).is_ok());
        assert_eq!(
            rehydrate(tags(11)),
            Err(QuizListError::TooManyTags { count: 11 })
        );
    }

    #[test]
    fn rejected_tag_change_leaves_quiz_list_unchanged() {
        let mut quiz_list = new_quiz_list(quiz_ids(1), tags(1)).unwrap();
        let before = quiz_list.clone();

        assert_eq!(
            quiz_list.change_tags(tags(11)),
            Err(ChangeTagsError::TooManyTags { count: 11 })
        );
        assert_eq!(quiz_list, before);
    }
}
