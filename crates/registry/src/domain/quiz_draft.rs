use std::collections::BTreeSet;

use isolang::Language;
use mitsein::vec1::Vec1;

use super::{
    media::Media,
    quiz::{CanonicalAnswerError, PromptTextError, QuizContent},
    response_mode::{CharacterChoiceError, DraftResponseMode, ImageChoiceError, TextChoiceError},
    tag::Tag,
};

/// The content of a quiz draft.
///
/// Drafts may be incomplete. Text may be blank, response modes may be missing
/// or duplicated, and the tag limit is not enforced.
///
/// Text length limits and NUL characters are checked at construction. All other
/// constraints are checked by [`QuizDraftContent::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuizDraftContent {
    /// The question text. May be blank.
    prompt_text: String,

    /// Media presented alongside the question text.
    prompt_media: Option<Media>,

    /// The canonical answer. May be blank.
    canonical_answer: String,

    /// Response modes. May be empty or contain duplicates.
    response_modes: Vec<DraftResponseMode>,

    /// The language of the quiz.
    language: Language,

    /// Tags. The limit is not enforced here.
    tags: BTreeSet<Tag>,
}

impl QuizDraftContent {
    /// Fails only on text length limits and NUL, see [`QuizDraftContentError`].
    pub(crate) fn new(
        prompt_text: impl Into<String>,
        prompt_media: Option<Media>,
        canonical_answer: impl Into<String>,
        response_modes: Vec<DraftResponseMode>,
        language: Language,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, QuizDraftContentError> {
        let _ = (
            prompt_text.into(),
            prompt_media,
            canonical_answer.into(),
            response_modes,
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

    pub(crate) fn response_modes(&self) -> &[DraftResponseMode] {
        &self.response_modes
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
    pub(crate) fn validate(&self) -> Result<QuizContent, Vec1<ValidationError>> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QuizDraftContentError {
    #[error(
        "prompt text must be at most {} characters, got {length}",
        super::quiz::PromptText::MAX_LENGTH
    )]
    PromptTextTooLong { length: usize },
    #[error("prompt text must not contain NUL")]
    PromptTextInvalidCharacter,
    #[error(
        "canonical answer must be at most {} characters, got {length}",
        super::quiz::CanonicalAnswer::MAX_LENGTH
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
pub(crate) enum ValidationError {
    #[error(transparent)]
    PromptText(PromptTextError),
    #[error(transparent)]
    CanonicalAnswer(CanonicalAnswerError),
    #[error("a quiz must support at least one response mode")]
    NoResponseMode,
    #[error("a quiz must not support the same kind of response mode twice")]
    DuplicateResponseMode,
    #[error("a quiz must have at most {} tags, got {count}", QuizContent::MAX_TAGS)]
    TooManyTags { count: usize },
    #[error("a free input must have at least one expected answer")]
    FreeInputNoExpectedAnswer,
    #[error("a text choice must have at least one correct answer")]
    TextChoiceNoCorrectAnswer,
    #[error(transparent)]
    TextChoice(TextChoiceError),
    #[error("an image choice must have at least one correct answer")]
    ImageChoiceNoCorrectAnswer,
    #[error(transparent)]
    ImageChoice(ImageChoiceError),
    #[error("character choices must have at least one position")]
    CharacterChoicesNoPosition,
    #[error("character choice at position {position} must have a correct character")]
    CharacterChoiceNoCorrectCharacter { position: usize },
    #[error("character choice at position {position} must have at least one wrong character")]
    CharacterChoiceNoWrongCharacter { position: usize },
    #[error("character choice at position {position}: {error}")]
    CharacterChoice {
        position: usize,
        error: CharacterChoiceError,
    },
}
