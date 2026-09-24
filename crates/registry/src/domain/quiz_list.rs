use std::collections::BTreeSet;

use uuid::Uuid;

use super::{quiz::QuizId, tag::Tag, user::UserId};

/// Uniquely identifies a quiz list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizListId(Uuid);

/// The title of a quiz list.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizListTitle(String);

/// A description of what a quiz list contains.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizListDescription(String);

/// A curated, ordered list of quizzes.
///
/// A quiz list refers to quizzes by identifier and does not own them. The same
/// quiz may appear in many lists, including lists owned by users other than its
/// author, and deleting a list leaves its quizzes untouched.
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
