use super::{quiz::QuizId, user::UserId};

/// Who may use a quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum AllowedUsers {
    /// Only the author.
    AuthorOnly,

    /// Anyone.
    Anyone,
}

/// Defines who may use a published quiz.
///
/// Created when the quiz is first published and deleted with it. The current
/// policy is checked when a quiz is added to a quiz list and again when a
/// session builds its lineup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuizUsePolicy {
    /// The quiz this policy applies to.
    quiz_id: QuizId,

    /// The quiz's author. Never changes.
    author: UserId,

    /// Who may use the quiz.
    allowed: AllowedUsers,
}

impl QuizUsePolicy {
    pub(crate) fn new(quiz_id: QuizId, author: UserId, allowed: AllowedUsers) -> Self {
        Self {
            quiz_id,
            author,
            allowed,
        }
    }

    /// Rehydrates a use policy from persisted state.
    ///
    /// For repository implementations only. There are no invariants to check,
    /// so this cannot fail.
    pub(crate) fn from_persistence(quiz_id: QuizId, author: UserId, allowed: AllowedUsers) -> Self {
        Self {
            quiz_id,
            author,
            allowed,
        }
    }

    pub(crate) fn quiz_id(&self) -> QuizId {
        self.quiz_id
    }

    pub(crate) fn author(&self) -> UserId {
        self.author
    }

    pub(crate) fn allowed(&self) -> AllowedUsers {
        self.allowed
    }

    /// Changes who may use the quiz. Takes effect immediately, whether or not a
    /// draft exists.
    pub(crate) fn change(&mut self, allowed: AllowedUsers) {
        let _ = allowed;
        todo!()
    }

    /// Whether `user` may use the quiz.
    ///
    /// The author can always use it. Other users can only use it when the
    /// policy is [`AllowedUsers::Anyone`].
    pub(crate) fn usable_by(&self, user: UserId) -> bool {
        let _ = user;
        todo!()
    }
}
