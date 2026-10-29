use std::collections::BTreeSet;

use isolang::Language;
use mitsein::vec1::Vec1;
use uuid::Uuid;

use super::{
    media::Media,
    response_mode::{ResponseMode, ResponseModeValidateError},
    tag::Tag,
    user::UserId,
};

/// Uniquely identifies a quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizId(Uuid);

impl QuizId {
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
/// The question text presented to players.
///
/// Guaranteed to be at most 1000 characters and free of NUL. May be blank,
/// which only a draft is allowed to be. Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct PromptText(String);

impl PromptText {
    pub(crate) const MAX_LENGTH: usize = 1000;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, PromptTextError> {
        let text = text.into();
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(PromptTextError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(PromptTextError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether the text is empty or only whitespace.
    pub(crate) fn is_blank(&self) -> bool {
        self.0.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PromptTextError {
    #[error(
        "prompt text must be at most {} characters, got {length}",
        PromptText::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("prompt text must not contain NUL")]
    InvalidCharacter,
}

/// The question presented to players.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Prompt {
    /// The question text.
    text: PromptText,

    /// Media presented alongside the text.
    media: Option<Media>,
}

impl Prompt {
    pub(crate) fn new(text: PromptText, media: Option<Media>) -> Self {
        Self { text, media }
    }

    pub(crate) fn text(&self) -> &PromptText {
        &self.text
    }

    pub(crate) fn media(&self) -> Option<Media> {
        self.media
    }
}

/// The canonical answer shown to players when the answer is revealed.
///
/// This is intended for presentation and does not necessarily correspond
/// exactly to the representation accepted by a particular [`ResponseMode`].
///
/// Guaranteed to be at most 200 characters and free of NUL. May be blank, which
/// only a draft is allowed to be. Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CanonicalAnswer(String);

impl CanonicalAnswer {
    pub(crate) const MAX_LENGTH: usize = 200;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, CanonicalAnswerError> {
        let text = text.into();
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(CanonicalAnswerError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(CanonicalAnswerError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether the text is empty or only whitespace.
    pub(crate) fn is_blank(&self) -> bool {
        self.0.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CanonicalAnswerError {
    #[error(
        "canonical answer must be at most {} characters, got {length}",
        CanonicalAnswer::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("canonical answer must not contain NUL")]
    InvalidCharacter,
}

/// A [`QuizContent`] that may still be incomplete.
///
/// Text may be blank, the response mode may be missing or incomplete, and the
/// tag limit is not enforced. [`DraftQuizContent::validate`] checks all of
/// this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftQuizContent {
    /// The question text.
    prompt_text: PromptText,

    /// Media presented alongside the question text.
    prompt_media: Option<Media>,

    /// The canonical answer.
    canonical_answer: CanonicalAnswer,

    /// The response mode.
    response_mode: Option<ResponseMode>,

    /// The language of the quiz.
    language: Language,

    /// Tags.
    tags: BTreeSet<Tag>,
}

impl DraftQuizContent {
    pub(crate) fn new(
        prompt_text: PromptText,
        prompt_media: Option<Media>,
        canonical_answer: CanonicalAnswer,
        response_mode: Option<ResponseMode>,
        language: Language,
        tags: BTreeSet<Tag>,
    ) -> Self {
        Self {
            prompt_text,
            prompt_media,
            canonical_answer,
            response_mode,
            language,
            tags,
        }
    }

    pub(crate) fn prompt_text(&self) -> &PromptText {
        &self.prompt_text
    }

    pub(crate) fn prompt_media(&self) -> Option<Media> {
        self.prompt_media
    }

    pub(crate) fn canonical_answer(&self) -> &CanonicalAnswer {
        &self.canonical_answer
    }

    pub(crate) fn response_mode(&self) -> Option<&ResponseMode> {
        self.response_mode.as_ref()
    }

    pub(crate) fn language(&self) -> Language {
        self.language
    }

    pub(crate) fn tags(&self) -> &BTreeSet<Tag> {
        &self.tags
    }

    /// Validates all fields and returns content ready for publication.
    ///
    /// Returns all violations instead of stopping at the first one.
    pub(crate) fn validate(&self) -> Result<QuizContent, Vec1<DraftQuizContentValidateError>> {
        let mut errors = Vec::new();
        if self.prompt_text.is_blank() {
            errors.push(DraftQuizContentValidateError::BlankPromptText);
        }
        if self.canonical_answer.is_blank() {
            errors.push(DraftQuizContentValidateError::BlankCanonicalAnswer);
        }
        match &self.response_mode {
            Some(response_mode) => {
                if let Err(mode_errors) = response_mode.validate() {
                    errors.extend(
                        mode_errors
                            .into_iter()
                            .map(DraftQuizContentValidateError::InvalidResponseMode),
                    );
                }
            }
            None => errors.push(DraftQuizContentValidateError::NoResponseMode),
        }
        let count = self.tags.len();
        if count > QuizContent::MAX_TAGS {
            errors.push(DraftQuizContentValidateError::TooManyTags { count });
        }
        if let Ok(errors) = Vec1::try_from(errors) {
            return Err(errors);
        }

        let response_mode = self
            .response_mode
            .clone()
            .expect("a missing response mode was reported above");
        let content = QuizContent::new(
            Prompt::new(self.prompt_text.clone(), self.prompt_media),
            self.canonical_answer.clone(),
            response_mode,
            self.language,
            self.tags.clone(),
        )
        .expect("the checks above already passed");
        Ok(content)
    }
}

/// A reason a draft cannot be published.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum DraftQuizContentValidateError {
    #[error("prompt text must not be empty or only whitespace")]
    BlankPromptText,
    #[error("canonical answer must not be empty or only whitespace")]
    BlankCanonicalAnswer,
    #[error("a quiz must have a response mode")]
    NoResponseMode,
    #[error("a quiz must have at most {} tags, got {count}", QuizContent::MAX_TAGS)]
    TooManyTags { count: usize },
    #[error("the response mode is not ready for publication")]
    InvalidResponseMode(ResponseModeValidateError),
}

/// Identifies one published version of a quiz's content.
///
/// Assigned on every publication, so it also serves as a version identifier,
/// distinguishing versions even when their content is identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct QuizContentId(Uuid);

impl QuizContentId {
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

/// One published version of a quiz's content.
///
/// Guaranteed to carry a prompt text and a canonical answer that are not blank,
/// a response mode ready for publication and at most 10 tags. Session-specific
/// behavior, such as how free-input responses are judged, is configured outside
/// the aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuizContent {
    /// Identifies this version.
    id: QuizContentId,

    /// The question presented to players.
    prompt: Prompt,

    /// The canonical answer displayed when the answer is revealed.
    canonical_answer: CanonicalAnswer,

    /// The way players respond to this quiz.
    response_mode: ResponseMode,

    /// The language in which this quiz is authored.
    language: Language,

    /// Tags associated with this quiz.
    tags: BTreeSet<Tag>,
}

impl QuizContent {
    pub(crate) const MAX_TAGS: usize = 10;

    fn check_prompt(prompt: &Prompt) -> Result<(), QuizContentError> {
        if prompt.text().is_blank() {
            return Err(QuizContentError::BlankPromptText);
        }
        Ok(())
    }

    fn check_canonical_answer(canonical_answer: &CanonicalAnswer) -> Result<(), QuizContentError> {
        if canonical_answer.is_blank() {
            return Err(QuizContentError::BlankCanonicalAnswer);
        }
        Ok(())
    }

    fn check_response_mode(response_mode: &ResponseMode) -> Result<(), QuizContentError> {
        response_mode
            .validate()
            .map_err(QuizContentError::InvalidResponseMode)
    }

    fn check_tags(tags: &BTreeSet<Tag>) -> Result<(), QuizContentError> {
        let count = tags.len();
        if count > Self::MAX_TAGS {
            return Err(QuizContentError::TooManyTags { count });
        }
        Ok(())
    }

    /// Creates a new version of the content with a freshly generated
    /// identifier.
    pub(crate) fn new(
        prompt: Prompt,
        canonical_answer: CanonicalAnswer,
        response_mode: ResponseMode,
        language: Language,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, QuizContentError> {
        Self::check_prompt(&prompt)?;
        Self::check_canonical_answer(&canonical_answer)?;
        Self::check_response_mode(&response_mode)?;
        Self::check_tags(&tags)?;
        Ok(Self {
            id: QuizContentId::new(),
            prompt,
            canonical_answer,
            response_mode,
            language,
            tags,
        })
    }

    /// Rehydrates a version of the content from persisted state.
    ///
    /// For repository implementations only. Checks the invariants, which hold
    /// for every version however it was obtained.
    pub(crate) fn from_persistence(
        id: QuizContentId,
        prompt: Prompt,
        canonical_answer: CanonicalAnswer,
        response_mode: ResponseMode,
        language: Language,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, QuizContentError> {
        Self::check_prompt(&prompt)?;
        Self::check_canonical_answer(&canonical_answer)?;
        Self::check_response_mode(&response_mode)?;
        Self::check_tags(&tags)?;
        Ok(Self {
            id,
            prompt,
            canonical_answer,
            response_mode,
            language,
            tags,
        })
    }

    pub(crate) fn id(&self) -> QuizContentId {
        self.id
    }

    pub(crate) fn prompt(&self) -> &Prompt {
        &self.prompt
    }

    pub(crate) fn canonical_answer(&self) -> &CanonicalAnswer {
        &self.canonical_answer
    }

    pub(crate) fn response_mode(&self) -> &ResponseMode {
        &self.response_mode
    }

    pub(crate) fn language(&self) -> Language {
        self.language
    }

    pub(crate) fn tags(&self) -> &BTreeSet<Tag> {
        &self.tags
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QuizContentError {
    #[error("prompt text must not be empty or only whitespace")]
    BlankPromptText,
    #[error("canonical answer must not be empty or only whitespace")]
    BlankCanonicalAnswer,
    #[error("the response mode is not ready for publication")]
    InvalidResponseMode(Vec1<ResponseModeValidateError>),
    #[error("a quiz must have at most {} tags, got {count}", QuizContent::MAX_TAGS)]
    TooManyTags { count: usize },
}
/// Whose quiz lists may include a quiz.
///
/// Sessions pose quizzes through quiz lists. This policy governs who may
/// redistribute the quiz through their own lists, not who may play it. A quiz
/// in a public list owned by its author can be played by anyone who can reach
/// that list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum InclusionPolicy {
    /// Only lists owned by the author.
    AuthorOnly,

    /// Lists owned by anyone.
    Anyone,
}

/// Where a quiz is in its publication lifecycle.
///
/// A quiz starts out unpublished with a draft. Publishing turns the draft into
/// the first content version. A published quiz may hold a draft of its next
/// version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum QuizState {
    /// Never published. Only a draft exists.
    Unpublished {
        /// The content being worked on.
        draft: DraftQuizContent,
    },

    /// Published at least once.
    Published {
        /// The latest published version.
        content: QuizContent,

        /// The content being worked on for the next version.
        draft: Option<DraftQuizContent>,
    },
}

/// A quiz authored by a user.
///
/// A quiz may be unpublished or published, see [`QuizState`]. It keeps its
/// identity across revisions. Each publication replaces the content with a new
/// version. Statistics use the quiz ID regardless of version. The inclusion
/// policy and the draft are not versioned.
///
/// Whether a user may edit a quiz is answered by [`Quiz::editable_by`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Quiz {
    /// The unique identifier of this quiz.
    id: QuizId,

    /// The author who created this quiz.
    author: UserId,

    /// Whose quiz lists may include this quiz.
    ///
    /// Checked when the quiz is added to a list and again when a session builds
    /// its lineup from one.
    inclusion_policy: InclusionPolicy,

    /// Where this quiz is in its publication lifecycle.
    state: QuizState,
}

impl Quiz {
    /// Starts a new, unpublished quiz with a freshly generated identifier.
    pub(crate) fn new(
        author: UserId,
        draft: DraftQuizContent,
        inclusion_policy: InclusionPolicy,
    ) -> Self {
        Self {
            id: QuizId::new(),
            author,
            inclusion_policy,
            state: QuizState::Unpublished { draft },
        }
    }

    /// Rehydrates a quiz from persisted state.
    ///
    /// For repository implementations only. The aggregate invariants are
    /// carried by [`QuizContent`] and [`QuizState`], so this cannot fail.
    pub(crate) fn from_persistence(
        id: QuizId,
        author: UserId,
        inclusion_policy: InclusionPolicy,
        state: QuizState,
    ) -> Self {
        Self {
            id,
            author,
            inclusion_policy,
            state,
        }
    }

    pub(crate) fn id(&self) -> QuizId {
        self.id
    }

    pub(crate) fn author(&self) -> UserId {
        self.author
    }

    pub(crate) fn inclusion_policy(&self) -> InclusionPolicy {
        self.inclusion_policy
    }

    pub(crate) fn state(&self) -> &QuizState {
        &self.state
    }

    /// Returns whether `user` may edit this quiz.
    ///
    /// Editing covers working on the draft, publishing, changing the inclusion
    /// policy and deleting the quiz. Only the author may edit.
    pub(crate) fn editable_by(&self, user: UserId) -> bool {
        user == self.author
    }

    /// Returns whether a quiz list owned by `list_owner` may include this quiz.
    ///
    /// The author always may. Anyone else may only under
    /// [`InclusionPolicy::Anyone`].
    pub(crate) fn includable_by(&self, list_owner: UserId) -> bool {
        list_owner == self.author || self.inclusion_policy == InclusionPolicy::Anyone
    }

    /// Replaces the draft. Starts one if the quiz has none.
    pub(crate) fn edit_draft(&mut self, draft: DraftQuizContent) {
        match &mut self.state {
            QuizState::Unpublished { draft: current } => *current = draft,
            QuizState::Published { draft: current, .. } => *current = Some(draft),
        }
    }

    /// Drops the draft of a published quiz.
    ///
    /// An unpublished quiz has nothing but its draft, so discarding that draft
    /// means deleting the quiz.
    pub(crate) fn discard_draft(&mut self) -> Result<(), DiscardDraftError> {
        match &mut self.state {
            QuizState::Unpublished { .. } => Err(DiscardDraftError::Unpublished),
            QuizState::Published { draft, .. } => match draft.take() {
                Some(_) => Ok(()),
                None => Err(DiscardDraftError::NoDraft),
            },
        }
    }

    /// Publishes the draft as the next content version.
    ///
    /// For an unpublished quiz this creates the first version. The draft is
    /// consumed.
    pub(crate) fn publish(&mut self) -> Result<(), PublishError> {
        let draft = match &self.state {
            QuizState::Unpublished { draft } => draft,
            QuizState::Published {
                draft: Some(draft), ..
            } => draft,
            QuizState::Published { draft: None, .. } => return Err(PublishError::NoDraft),
        };
        let content = draft.validate().map_err(PublishError::Invalid)?;
        self.state = QuizState::Published {
            content,
            draft: None,
        };
        Ok(())
    }

    /// Changes whose lists may include this quiz.
    ///
    /// Takes effect immediately and does not involve the draft. Inclusions that
    /// are no longer permitted are skipped when a session builds its lineup.
    pub(crate) fn change_inclusion_policy(&mut self, inclusion_policy: InclusionPolicy) {
        self.inclusion_policy = inclusion_policy;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum DiscardDraftError {
    #[error("the quiz has no draft")]
    NoDraft,
    #[error("the quiz has not been published")]
    Unpublished,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PublishError {
    #[error("the quiz has no draft")]
    NoDraft,
    #[error("the draft is not ready for publication")]
    Invalid(Vec1<DraftQuizContentValidateError>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::response_mode::{FreeInput, FreeInputAnswer};

    fn free_input() -> ResponseMode {
        ResponseMode::FreeInput(
            FreeInput::new(BTreeSet::from([FreeInputAnswer::new("Tokyo").unwrap()])).unwrap(),
        )
    }

    fn tags(count: usize) -> BTreeSet<Tag> {
        (0..count)
            .map(|index| Tag::new(format!("tag{index}")).unwrap())
            .collect()
    }

    fn new_content(
        prompt_text: &str,
        canonical_answer: &str,
        response_mode: ResponseMode,
        tags: BTreeSet<Tag>,
    ) -> Result<QuizContent, QuizContentError> {
        QuizContent::new(
            Prompt::new(PromptText::new(prompt_text).unwrap(), None),
            CanonicalAnswer::new(canonical_answer).unwrap(),
            response_mode,
            Language::Eng,
            tags,
        )
    }

    fn complete_content(
        response_mode: ResponseMode,
        tags: BTreeSet<Tag>,
    ) -> Result<QuizContent, QuizContentError> {
        new_content(
            "What is the capital of Japan?",
            "Tokyo",
            response_mode,
            tags,
        )
    }

    #[test]
    fn accepts_text_within_bounds() {
        assert!(PromptText::new("x".repeat(1000)).is_ok());
        assert!(CanonicalAnswer::new("x".repeat(200)).is_ok());
    }

    #[test]
    fn accepts_blank_text() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
            assert!(PromptText::new(text).unwrap().is_blank(), "{text:?}");
            assert!(CanonicalAnswer::new(text).unwrap().is_blank(), "{text:?}");
        }
        assert!(!PromptText::new(" x ").unwrap().is_blank());
    }

    #[test]
    fn rejects_blank_text_over_the_limit() {
        assert_eq!(
            PromptText::new(" ".repeat(1001)),
            Err(PromptTextError::TooLong { length: 1001 })
        );
    }

    #[test]
    fn rejects_text_over_the_limit() {
        assert_eq!(
            PromptText::new("x".repeat(1001)),
            Err(PromptTextError::TooLong { length: 1001 })
        );
        assert_eq!(
            CanonicalAnswer::new("x".repeat(201)),
            Err(CanonicalAnswerError::TooLong { length: 201 })
        );
    }

    #[test]
    fn counts_length_in_characters_not_bytes() {
        assert!(PromptText::new("あ".repeat(1000)).is_ok());
        assert_eq!(
            PromptText::new("あ".repeat(1001)),
            Err(PromptTextError::TooLong { length: 1001 })
        );
    }

    #[test]
    fn rejects_text_containing_nul() {
        for text in ["\0", "a\0", "\0a", "a\0b"] {
            assert_eq!(
                PromptText::new(text),
                Err(PromptTextError::InvalidCharacter),
                "{text:?}"
            );
            assert_eq!(
                CanonicalAnswer::new(text),
                Err(CanonicalAnswerError::InvalidCharacter),
                "{text:?}"
            );
        }
    }

    #[test]
    fn preserves_accepted_text_exactly() {
        for text in [" padded ", "Tokyo", "東京", "a\u{301}"] {
            assert_eq!(PromptText::new(text).unwrap().as_str(), text);
        }
    }

    #[test]
    fn creates_content_with_ten_tags() {
        let content = complete_content(free_input(), tags(10)).unwrap();

        assert_eq!(content.tags().len(), 10);
    }

    #[test]
    fn rejects_blank_content_text() {
        assert_eq!(
            new_content(" ", "Tokyo", free_input(), tags(0)),
            Err(QuizContentError::BlankPromptText)
        );
        assert_eq!(
            new_content("Q", "", free_input(), tags(0)),
            Err(QuizContentError::BlankCanonicalAnswer)
        );
    }

    #[test]
    fn rejects_more_than_ten_tags() {
        assert_eq!(
            complete_content(free_input(), tags(11)),
            Err(QuizContentError::TooManyTags { count: 11 })
        );
    }

    #[test]
    fn rejects_an_incomplete_response_mode() {
        let free_input = ResponseMode::FreeInput(FreeInput::new(BTreeSet::new()).unwrap());

        assert_eq!(
            complete_content(free_input, tags(0)),
            Err(QuizContentError::InvalidResponseMode(Vec1::from_one(
                ResponseModeValidateError::NoExpectedAnswer
            )))
        );
    }

    fn user() -> UserId {
        UserId::from_uuid(Uuid::now_v7())
    }

    fn new_draft(
        prompt_text: &str,
        canonical_answer: &str,
        response_mode: Option<ResponseMode>,
        tags: BTreeSet<Tag>,
    ) -> DraftQuizContent {
        DraftQuizContent::new(
            PromptText::new(prompt_text).unwrap(),
            None,
            CanonicalAnswer::new(canonical_answer).unwrap(),
            response_mode,
            Language::Eng,
            tags,
        )
    }

    fn complete_draft() -> DraftQuizContent {
        new_draft(
            "What is the capital of Japan?",
            "Tokyo",
            Some(free_input()),
            tags(1),
        )
    }

    fn unpublished_quiz(author: UserId) -> Quiz {
        Quiz::new(author, complete_draft(), InclusionPolicy::AuthorOnly)
    }

    fn published_quiz(author: UserId) -> Quiz {
        let mut quiz = unpublished_quiz(author);
        quiz.publish().unwrap();
        quiz
    }

    fn published_content(quiz: &Quiz) -> &QuizContent {
        match quiz.state() {
            QuizState::Published { content, .. } => content,
            state => panic!("expected a published quiz, got {state:?}"),
        }
    }

    #[test]
    fn validates_a_complete_draft() {
        let content = complete_draft().validate().unwrap();

        assert_eq!(
            content.prompt().text().as_str(),
            "What is the capital of Japan?"
        );
        assert_eq!(content.canonical_answer().as_str(), "Tokyo");
        assert_eq!(content.response_mode(), &free_input());
        assert_eq!(content.language(), Language::Eng);
        assert_eq!(content.tags(), &tags(1));
    }

    #[test]
    fn reports_every_violation_of_a_draft() {
        let draft = new_draft(" ", "", None, tags(11));

        assert_eq!(
            draft.validate(),
            Err(Vec1::from_head_and_tail(
                DraftQuizContentValidateError::BlankPromptText,
                [
                    DraftQuizContentValidateError::BlankCanonicalAnswer,
                    DraftQuizContentValidateError::NoResponseMode,
                    DraftQuizContentValidateError::TooManyTags { count: 11 },
                ]
            ))
        );
    }

    #[test]
    fn reports_an_invalid_response_mode_of_a_draft() {
        let free_input = ResponseMode::FreeInput(FreeInput::new(BTreeSet::new()).unwrap());
        let draft = new_draft("Q", "A", Some(free_input), tags(0));

        assert_eq!(
            draft.validate(),
            Err(Vec1::from_one(
                DraftQuizContentValidateError::InvalidResponseMode(
                    ResponseModeValidateError::NoExpectedAnswer
                )
            ))
        );
    }

    #[test]
    fn only_the_author_may_edit() {
        let author = user();
        let quiz = unpublished_quiz(author);

        assert!(quiz.editable_by(author));
        assert!(!quiz.editable_by(user()));
    }

    #[test]
    fn inclusion_follows_the_policy() {
        let author = user();
        let other = user();
        let mut quiz = unpublished_quiz(author);

        assert!(quiz.includable_by(author));
        assert!(!quiz.includable_by(other));

        quiz.change_inclusion_policy(InclusionPolicy::Anyone);

        assert_eq!(quiz.inclusion_policy(), InclusionPolicy::Anyone);
        assert!(quiz.includable_by(author));
        assert!(quiz.includable_by(other));
    }

    #[test]
    fn publishes_the_first_version() {
        let mut quiz = unpublished_quiz(user());

        quiz.publish().unwrap();

        assert!(matches!(
            quiz.state(),
            QuizState::Published { draft: None, .. }
        ));
        assert_eq!(
            published_content(&quiz).canonical_answer().as_str(),
            "Tokyo"
        );
    }

    #[test]
    fn publishes_the_draft_as_a_new_version() {
        let mut quiz = published_quiz(user());
        let first_version = published_content(&quiz).id();
        quiz.edit_draft(new_draft("Q", "Kyoto", Some(free_input()), tags(0)));

        quiz.publish().unwrap();

        assert!(matches!(
            quiz.state(),
            QuizState::Published { draft: None, .. }
        ));
        assert_ne!(published_content(&quiz).id(), first_version);
        assert_eq!(
            published_content(&quiz).canonical_answer().as_str(),
            "Kyoto"
        );
    }

    #[test]
    fn publishing_an_invalid_draft_reports_the_violations() {
        let draft = new_draft("", "Tokyo", Some(free_input()), tags(0));
        let mut quiz = Quiz::new(user(), draft, InclusionPolicy::AuthorOnly);
        let before = quiz.clone();

        assert_eq!(
            quiz.publish(),
            Err(PublishError::Invalid(Vec1::from_one(
                DraftQuizContentValidateError::BlankPromptText
            )))
        );
        assert_eq!(quiz, before);
    }

    #[test]
    fn publishing_without_a_draft_fails() {
        let mut quiz = published_quiz(user());

        assert_eq!(quiz.publish(), Err(PublishError::NoDraft));
    }

    #[test]
    fn editing_replaces_the_draft() {
        let draft = new_draft("Q", "A", None, tags(0));
        let mut unpublished = unpublished_quiz(user());
        let mut published = published_quiz(user());

        unpublished.edit_draft(draft.clone());
        published.edit_draft(draft.clone());

        assert_eq!(
            unpublished.state(),
            &QuizState::Unpublished {
                draft: draft.clone()
            }
        );
        assert!(matches!(
            published.state(),
            QuizState::Published { draft: Some(current), .. } if *current == draft
        ));
    }

    #[test]
    fn discards_the_draft_of_a_published_quiz() {
        let mut quiz = published_quiz(user());
        quiz.edit_draft(new_draft("Q", "A", None, tags(0)));

        assert_eq!(quiz.discard_draft(), Ok(()));
        assert!(matches!(
            quiz.state(),
            QuizState::Published { draft: None, .. }
        ));
        assert_eq!(quiz.discard_draft(), Err(DiscardDraftError::NoDraft));
    }

    #[test]
    fn discarding_the_draft_of_an_unpublished_quiz_fails() {
        let mut quiz = unpublished_quiz(user());
        let before = quiz.clone();

        assert_eq!(quiz.discard_draft(), Err(DiscardDraftError::Unpublished));
        assert_eq!(quiz, before);
    }
}
