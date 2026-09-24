use std::collections::BTreeSet;

use uuid::Uuid;

use super::{quiz::QuizId, tag::Tag, user::UserId};

/// Uniquely identifies a quiz list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizListId(Uuid);

impl QuizListId {
    /// Generates a new, time-ordered identifier.
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

/// A curated, ordered list of quizzes.
///
/// A quiz list refers to quizzes by identifier and does not own them. The same
/// quiz may appear in many lists, including lists owned by users other than its
/// author, and deleting a list leaves its quizzes untouched.
///
/// Guaranteed to hold at most 100 quizzes with no quiz twice, and to carry at
/// most 10 tags. Whether a quiz exists or may be added, and whether a caller
/// may edit the list, is decided by the application layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuizList {
    /// The unique identifier of this quiz list.
    id: QuizListId,

    /// The user who owns this quiz list.
    owner: UserId,

    /// The title of this quiz list.
    title: QuizListTitle,

    /// An optional description of this quiz list.
    description: Option<QuizListDescription>,

    /// The quizzes in this list.
    ///
    /// The order is significant and may be empty. Deleting a quiz does not
    /// remove it from lists, so an identifier here may refer to a quiz that no
    /// longer exists.
    quiz_ids: Vec<QuizId>,

    /// Tags associated with this quiz list.
    tags: BTreeSet<Tag>,
}

impl QuizList {
    pub(crate) const MAX_QUIZZES: usize = 100;
    pub(crate) const MAX_TAGS: usize = 10;

    fn check_quiz_ids(quiz_ids: &[QuizId]) -> Result<(), QuizListError> {
        let count = quiz_ids.len();
        if count > Self::MAX_QUIZZES {
            return Err(QuizListError::TooManyQuizzes { count });
        }
        let mut seen = BTreeSet::new();
        for quiz_id in quiz_ids {
            if !seen.insert(quiz_id) {
                return Err(QuizListError::DuplicateQuiz);
            }
        }
        Ok(())
    }

    fn check_tags(tags: &BTreeSet<Tag>) -> Result<(), QuizListError> {
        let count = tags.len();
        if count > Self::MAX_TAGS {
            return Err(QuizListError::TooManyTags { count });
        }
        Ok(())
    }

    /// Creates a new quiz list with a freshly generated identifier.
    pub(crate) fn new(
        owner: UserId,
        title: QuizListTitle,
        description: Option<QuizListDescription>,
        quiz_ids: Vec<QuizId>,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, QuizListError> {
        Self::check_quiz_ids(&quiz_ids)?;
        Self::check_tags(&tags)?;
        Ok(Self {
            id: QuizListId::new(),
            owner,
            title,
            description,
            quiz_ids,
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
        quiz_ids: Vec<QuizId>,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, QuizListError> {
        Self::check_quiz_ids(&quiz_ids)?;
        Self::check_tags(&tags)?;
        Ok(Self {
            id,
            owner,
            title,
            description,
            quiz_ids,
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

    pub(crate) fn quiz_ids(&self) -> &[QuizId] {
        &self.quiz_ids
    }

    pub(crate) fn tags(&self) -> &BTreeSet<Tag> {
        &self.tags
    }

    pub(crate) fn change_title(&mut self, title: QuizListTitle) {
        self.title = title;
    }

    pub(crate) fn change_description(&mut self, description: Option<QuizListDescription>) {
        self.description = description;
    }

    /// Appends a quiz to the end of the list. Fails with
    /// [`QuizListError::DuplicateQuiz`] or [`QuizListError::TooManyQuizzes`].
    pub(crate) fn add_quiz(&mut self, quiz_id: QuizId) -> Result<(), QuizListError> {
        if self.quiz_ids.contains(&quiz_id) {
            return Err(QuizListError::DuplicateQuiz);
        }
        let count = self.quiz_ids.len() + 1;
        if count > Self::MAX_QUIZZES {
            return Err(QuizListError::TooManyQuizzes { count });
        }
        self.quiz_ids.push(quiz_id);
        Ok(())
    }

    /// Removes a quiz from the list. Does nothing if the quiz is not in it.
    pub(crate) fn remove_quiz(&mut self, quiz_id: QuizId) {
        self.quiz_ids.retain(|id| *id != quiz_id);
    }

    /// Replaces the quizzes, for example to reorder them. Fails with
    /// [`QuizListError::DuplicateQuiz`] or [`QuizListError::TooManyQuizzes`].
    pub(crate) fn change_quiz_ids(&mut self, quiz_ids: Vec<QuizId>) -> Result<(), QuizListError> {
        Self::check_quiz_ids(&quiz_ids)?;
        self.quiz_ids = quiz_ids;
        Ok(())
    }

    /// Replaces the tags. Fails only with [`QuizListError::TooManyTags`].
    pub(crate) fn change_tags(&mut self, tags: BTreeSet<Tag>) -> Result<(), QuizListError> {
        Self::check_tags(&tags)?;
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
        QuizList::MAX_QUIZZES
    )]
    TooManyQuizzes { count: usize },
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
    fn creates_empty_quiz_list() {
        let quiz_list = new_quiz_list(Vec::new(), tags(0)).unwrap();

        assert!(quiz_list.quiz_ids().is_empty());
    }

    #[test]
    fn creates_quiz_list_at_the_limits() {
        let quiz_list = new_quiz_list(quiz_ids(100), tags(10)).unwrap();

        assert_eq!(quiz_list.quiz_ids().len(), 100);
        assert_eq!(quiz_list.tags().len(), 10);
    }

    #[test]
    fn rejects_duplicate_quiz() {
        let quiz_id = QuizId::new();

        assert_eq!(
            new_quiz_list(vec![quiz_id, QuizId::new(), quiz_id], tags(0)),
            Err(QuizListError::DuplicateQuiz)
        );
    }

    #[test]
    fn rejects_more_than_a_hundred_quizzes() {
        assert_eq!(
            new_quiz_list(quiz_ids(101), tags(0)),
            Err(QuizListError::TooManyQuizzes { count: 101 })
        );
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
        let rehydrate = |quiz_ids, tags| {
            QuizList::from_persistence(
                QuizListId::new(),
                UserId::from_uuid(Uuid::now_v7()),
                QuizListTitle::new("Capitals").unwrap(),
                None,
                quiz_ids,
                tags,
            )
        };
        let quiz_id = QuizId::new();

        assert!(rehydrate(quiz_ids(100), tags(10)).is_ok());
        assert_eq!(
            rehydrate(vec![quiz_id, quiz_id], tags(0)),
            Err(QuizListError::DuplicateQuiz)
        );
        assert_eq!(
            rehydrate(quiz_ids(101), tags(0)),
            Err(QuizListError::TooManyQuizzes { count: 101 })
        );
        assert_eq!(
            rehydrate(Vec::new(), tags(11)),
            Err(QuizListError::TooManyTags { count: 11 })
        );
    }

    #[test]
    fn adds_quizzes_in_order() {
        let mut quiz_list = new_quiz_list(Vec::new(), tags(0)).unwrap();
        let first = QuizId::new();
        let second = QuizId::new();

        quiz_list.add_quiz(first).unwrap();
        quiz_list.add_quiz(second).unwrap();

        assert_eq!(quiz_list.quiz_ids(), [first, second]);
    }

    #[test]
    fn rejected_add_leaves_quiz_list_unchanged() {
        let quiz_id = QuizId::new();
        let mut quiz_list = new_quiz_list(vec![quiz_id], tags(0)).unwrap();
        assert_eq!(
            quiz_list.add_quiz(quiz_id),
            Err(QuizListError::DuplicateQuiz)
        );
        assert_eq!(quiz_list.quiz_ids(), [quiz_id]);

        let mut full = new_quiz_list(quiz_ids(100), tags(0)).unwrap();
        let before = full.clone();
        assert_eq!(
            full.add_quiz(QuizId::new()),
            Err(QuizListError::TooManyQuizzes { count: 101 })
        );
        assert_eq!(full, before);
    }

    #[test]
    fn removes_quiz_and_ignores_missing_one() {
        let first = QuizId::new();
        let second = QuizId::new();
        let mut quiz_list = new_quiz_list(vec![first, second], tags(0)).unwrap();

        quiz_list.remove_quiz(first);
        quiz_list.remove_quiz(QuizId::new());

        assert_eq!(quiz_list.quiz_ids(), [second]);
    }

    #[test]
    fn reorders_quizzes() {
        let first = QuizId::new();
        let second = QuizId::new();
        let mut quiz_list = new_quiz_list(vec![first, second], tags(0)).unwrap();

        quiz_list.change_quiz_ids(vec![second, first]).unwrap();

        assert_eq!(quiz_list.quiz_ids(), [second, first]);
    }

    #[test]
    fn rejected_change_leaves_quiz_list_unchanged() {
        let quiz_id = QuizId::new();
        let mut quiz_list = new_quiz_list(vec![quiz_id], tags(1)).unwrap();
        let before = quiz_list.clone();

        assert_eq!(
            quiz_list.change_quiz_ids(vec![quiz_id, quiz_id]),
            Err(QuizListError::DuplicateQuiz)
        );
        assert_eq!(
            quiz_list.change_tags(tags(11)),
            Err(QuizListError::TooManyTags { count: 11 })
        );
        assert_eq!(quiz_list, before);
    }
}
