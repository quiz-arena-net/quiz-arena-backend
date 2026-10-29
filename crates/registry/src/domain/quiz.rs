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
/// Guaranteed to be at most 200 characters and free of NUL. May be blank,
/// which only a draft is allowed to be. Accepted text is preserved exactly.
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
        todo!()
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
/// Guaranteed to carry a prompt text and a canonical answer that are not
/// blank, a response mode ready for publication and at most 10 tags.
/// Session-specific behavior, such as how free-input responses are judged, is
/// configured outside the aggregate.
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
        let _ = user;
        todo!()
    }

    /// Returns whether a quiz list owned by `list_owner` may include this quiz.
    ///
    /// The author always may. Anyone else may only under
    /// [`InclusionPolicy::Anyone`].
    pub(crate) fn includable_by(&self, list_owner: UserId) -> bool {
        let _ = list_owner;
        todo!()
    }

    /// Replaces the draft. Starts one if the quiz has none.
    pub(crate) fn edit_draft(&mut self, draft: DraftQuizContent) {
        let _ = draft;
        todo!()
    }

    /// Drops the draft of a published quiz.
    ///
    /// An unpublished quiz has nothing but its draft, so discarding that draft
    /// means deleting the quiz.
    pub(crate) fn discard_draft(&mut self) -> Result<(), DiscardDraftError> {
        todo!()
    }

    /// Publishes the draft as the next content version.
    ///
    /// For an unpublished quiz this creates the first version. The draft is
    /// consumed.
    pub(crate) fn publish(&mut self) -> Result<(), PublishError> {
        todo!()
    }

    /// Changes whose lists may include this quiz.
    ///
    /// Takes effect immediately and does not involve the draft. Inclusions that
    /// are no longer permitted are skipped when a session builds its lineup.
    pub(crate) fn change_inclusion_policy(&mut self, inclusion_policy: InclusionPolicy) {
        let _ = inclusion_policy;
        todo!()
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
}
