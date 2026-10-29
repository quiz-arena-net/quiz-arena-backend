use std::collections::BTreeSet;

use isolang::Language;
use mitsein::vec1::Vec1;
use uuid::Uuid;

use super::{
    media::Media,
    response_mode::{DraftResponseMode, ImageChoice, ResponseMode, TextChoice},
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
/// Guaranteed to be 1 to 1000 characters, not only whitespace, and free of NUL.
/// Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct PromptText(String);

impl PromptText {
    pub(crate) const MAX_LENGTH: usize = 1000;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, PromptTextError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(PromptTextError::Blank);
        }
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
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PromptTextError {
    #[error("prompt text must not be empty or only whitespace")]
    Blank,
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
/// Guaranteed to be 1 to 200 characters, not only whitespace, and free of NUL.
/// Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CanonicalAnswer(String);

impl CanonicalAnswer {
    pub(crate) const MAX_LENGTH: usize = 200;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, CanonicalAnswerError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(CanonicalAnswerError::Blank);
        }
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
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CanonicalAnswerError {
    #[error("canonical answer must not be empty or only whitespace")]
    Blank,
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
/// Text may be blank, the response mode may be missing, and the tag limit is
/// not enforced.
///
/// Text length limits and NUL characters are checked at construction. All other
/// constraints are checked by [`DraftQuizContent::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftQuizContent {
    /// The question text.
    prompt_text: String,

    /// Media presented alongside the question text.
    prompt_media: Option<Media>,

    /// The canonical answer.
    canonical_answer: String,

    /// The response mode.
    response_mode: Option<DraftResponseMode>,

    /// The language of the quiz.
    language: Language,

    /// Tags.
    tags: BTreeSet<Tag>,
}

impl DraftQuizContent {
    pub(crate) fn new(
        prompt_text: impl Into<String>,
        prompt_media: Option<Media>,
        canonical_answer: impl Into<String>,
        response_mode: Option<DraftResponseMode>,
        language: Language,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, DraftQuizContentError> {
        let _ = (
            prompt_text.into(),
            prompt_media,
            canonical_answer.into(),
            response_mode,
            language,
            tags,
        );
        todo!()
    }

    pub(crate) fn prompt_text(&self) -> &str {
        &self.prompt_text
    }

    pub(crate) fn prompt_media(&self) -> Option<Media> {
        self.prompt_media
    }

    pub(crate) fn canonical_answer(&self) -> &str {
        &self.canonical_answer
    }

    pub(crate) fn response_mode(&self) -> Option<&DraftResponseMode> {
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum DraftQuizContentError {
    #[error(
        "prompt text must be at most {} characters, got {length}",
        PromptText::MAX_LENGTH
    )]
    PromptTextTooLong { length: usize },
    #[error("prompt text must not contain NUL")]
    PromptTextInvalidCharacter,
    #[error(
        "canonical answer must be at most {} characters, got {length}",
        CanonicalAnswer::MAX_LENGTH
    )]
    CanonicalAnswerTooLong { length: usize },
    #[error("canonical answer must not contain NUL")]
    CanonicalAnswerInvalidCharacter,
}

/// A reason a draft cannot be published.
///
/// Positions of character choices are zero-based indexes into
/// [`DraftCharacterChoices::choices`](super::response_mode::DraftCharacterChoices::choices).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum DraftQuizContentValidateError {
    #[error("prompt text must not be empty or only whitespace")]
    PromptTextBlank,
    #[error("canonical answer must not be empty or only whitespace")]
    CanonicalAnswerBlank,
    #[error("a quiz must have a response mode")]
    NoResponseMode,
    #[error("a quiz must have at most {} tags, got {count}", QuizContent::MAX_TAGS)]
    TooManyTags { count: usize },
    #[error("a free input must have at least one expected answer")]
    FreeInputNoExpectedAnswer,
    #[error("a text choice must have at least one correct answer")]
    TextChoiceNoCorrectAnswer,
    #[error("a text choice answer must not be both correct and wrong")]
    TextChoiceAnswerBothCorrectAndWrong,
    #[error(
        "a text choice must have at most {} answers, got {count}",
        TextChoice::MAX_ANSWERS
    )]
    TextChoiceTooManyAnswers { count: usize },
    #[error("an image choice must have at least one correct answer")]
    ImageChoiceNoCorrectAnswer,
    #[error("an image must not appear in more than one image choice answer")]
    ImageChoiceDuplicateImage,
    #[error(
        "an image choice must have at most {} answers, got {count}",
        ImageChoice::MAX_ANSWERS
    )]
    ImageChoiceTooManyAnswers { count: usize },
    #[error("character choices must have at least one position")]
    CharacterChoicesNoPosition,
    #[error("character choice at position {position} must have a correct character")]
    CharacterChoiceNoCorrectCharacter { position: usize },
    #[error("character choice at position {position} must have at least one wrong character")]
    CharacterChoiceNoWrongCharacter { position: usize },
    #[error(
        "character choice at position {position} must not have its correct character among the wrong ones"
    )]
    CharacterChoiceCorrectChoiceAmongWrongChoices { position: usize },
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
/// Guaranteed to carry at most 10 tags. Session-specific behavior, such as how
/// free-input responses are judged, is configured outside the aggregate.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QuizContentError {
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
    use mitsein::btree_set1::BTreeSet1;

    use super::*;
    use crate::domain::response_mode::{FreeInput, FreeInputAnswer};

    fn free_input() -> ResponseMode {
        ResponseMode::FreeInput(
            FreeInput::new(BTreeSet1::from_one(FreeInputAnswer::new("Tokyo").unwrap())).unwrap(),
        )
    }

    fn tags(count: usize) -> BTreeSet<Tag> {
        (0..count)
            .map(|index| Tag::new(format!("tag{index}")).unwrap())
            .collect()
    }

    fn new_content(
        response_mode: ResponseMode,
        tags: BTreeSet<Tag>,
    ) -> Result<QuizContent, QuizContentError> {
        QuizContent::new(
            Prompt::new(
                PromptText::new("What is the capital of Japan?").unwrap(),
                None,
            ),
            CanonicalAnswer::new("Tokyo").unwrap(),
            response_mode,
            Language::Eng,
            tags,
        )
    }

    #[test]
    fn accepts_text_within_bounds() {
        assert!(PromptText::new("x".repeat(1000)).is_ok());
        assert!(CanonicalAnswer::new("x".repeat(200)).is_ok());
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
    fn rejects_blank_text() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
            assert_eq!(
                PromptText::new(text),
                Err(PromptTextError::Blank),
                "{text:?}"
            );
            assert_eq!(
                CanonicalAnswer::new(text),
                Err(CanonicalAnswerError::Blank),
                "{text:?}"
            );
        }
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
        let content = new_content(free_input(), tags(10)).unwrap();

        assert_eq!(content.tags().len(), 10);
    }

    #[test]
    fn rejects_more_than_ten_tags() {
        assert_eq!(
            new_content(free_input(), tags(11)),
            Err(QuizContentError::TooManyTags { count: 11 })
        );
    }
}
