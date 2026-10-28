use std::{collections::BTreeSet, mem};

use isolang::Language;
use mitsein::vec1::Vec1;
use uuid::Uuid;

use super::{
    media::Media,
    quiz_draft::{QuizDraft, QuizDraftValidationError},
    response_mode::ResponseMode,
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

/// One published version of a quiz's content.
///
/// Guaranteed to support at most one response mode of each kind and to carry at
/// most 10 tags. Session-specific behavior, such as how free-input responses
/// are judged, is configured outside the aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuizContent {
    /// Identifies this version.
    id: QuizContentId,

    /// The question presented to players.
    prompt: Prompt,

    /// The canonical answer displayed when the answer is revealed.
    canonical_answer: CanonicalAnswer,

    /// Response modes supported by this quiz.
    ///
    /// The order may be used when presenting the available response modes.
    response_modes: Vec1<ResponseMode>,

    /// The language in which this quiz is authored.
    language: Language,

    /// Tags associated with this quiz.
    tags: BTreeSet<Tag>,
}

impl QuizContent {
    pub(crate) const MAX_TAGS: usize = 10;

    pub(crate) fn new(
        id: QuizContentId,
        prompt: Prompt,
        canonical_answer: CanonicalAnswer,
        response_modes: Vec1<ResponseMode>,
        language: Language,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, QuizContentError> {
        let mut seen_kinds = Vec::new();
        for response_mode in response_modes.iter() {
            let kind = mem::discriminant(response_mode);
            if seen_kinds.contains(&kind) {
                return Err(QuizContentError::DuplicateResponseMode);
            }
            seen_kinds.push(kind);
        }
        let count = tags.len();
        if count > Self::MAX_TAGS {
            return Err(QuizContentError::TooManyTags { count });
        }
        Ok(Self {
            id,
            prompt,
            canonical_answer,
            response_modes,
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

    pub(crate) fn response_modes(&self) -> &Vec1<ResponseMode> {
        &self.response_modes
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
    #[error("a quiz must not support the same kind of response mode twice")]
    DuplicateResponseMode,
    #[error("a quiz must have at most {} tags, got {count}", QuizContent::MAX_TAGS)]
    TooManyTags { count: usize },
}

/// A published quiz at a specific version of its content.
///
/// Each revision replaces the content, but keeps the same quiz ID. Statistics
/// use the quiz ID, regardless of version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Quiz {
    /// The unique identifier of this quiz.
    id: QuizId,

    /// The author who created this quiz.
    author: UserId,

    /// The content at the version this snapshot represents.
    content: QuizContent,
}

impl Quiz {
    /// Publishes a quiz for the first time.
    ///
    /// Consumes the draft and creates the first content version. Revising an
    /// already published quiz goes through [`Quiz::revise`] instead. The quiz's
    /// use policy is created alongside by the application layer.
    pub(crate) fn publish(draft: QuizDraft) -> Result<Self, Vec1<QuizDraftValidationError>> {
        let _ = draft;
        todo!()
    }

    /// Rehydrates a quiz from persisted state.
    ///
    /// For repository implementations only. The aggregate invariants are
    /// carried by [`QuizContent`], so this cannot fail.
    pub(crate) fn from_persistence(id: QuizId, author: UserId, content: QuizContent) -> Self {
        Self {
            id,
            author,
            content,
        }
    }

    pub(crate) fn id(&self) -> QuizId {
        self.id
    }

    pub(crate) fn author(&self) -> UserId {
        self.author
    }

    pub(crate) fn content(&self) -> &QuizContent {
        &self.content
    }

    /// Whether `user` may edit this quiz (revise it, change its use policy, or
    /// delete it).
    ///
    /// Only the author may do so.
    pub(crate) fn editable_by(&self, user: UserId) -> bool {
        let _ = user;
        todo!()
    }

    /// Publishes the draft as the next version of this quiz.
    ///
    /// Consumes the draft and replaces the content with a new version. The
    /// draft must belong to this quiz.
    pub(crate) fn revise(
        &mut self,
        draft: QuizDraft,
    ) -> Result<(), Vec1<QuizDraftValidationError>> {
        let _ = draft;
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use mitsein::btree_set1::BTreeSet1;

    use super::*;
    use crate::domain::response_mode::{
        ChoiceRequirement, FreeInput, FreeInputAnswer, TextChoice, TextChoiceAnswer,
    };

    fn free_input() -> ResponseMode {
        ResponseMode::FreeInput(FreeInput::new(BTreeSet1::from_one(
            FreeInputAnswer::new("Tokyo").unwrap(),
        )))
    }

    fn text_choice() -> ResponseMode {
        ResponseMode::TextChoice(
            TextChoice::new(
                ChoiceRequirement::One,
                BTreeSet1::from_one(TextChoiceAnswer::new("Tokyo").unwrap()),
                BTreeSet::from([TextChoiceAnswer::new("Kyoto").unwrap()]),
            )
            .unwrap(),
        )
    }

    fn tags(count: usize) -> BTreeSet<Tag> {
        (0..count)
            .map(|index| Tag::new(format!("tag{index}")).unwrap())
            .collect()
    }

    fn new_content(
        response_modes: Vec1<ResponseMode>,
        tags: BTreeSet<Tag>,
    ) -> Result<QuizContent, QuizContentError> {
        QuizContent::new(
            QuizContentId::new(),
            Prompt::new(
                PromptText::new("What is the capital of Japan?").unwrap(),
                None,
            ),
            CanonicalAnswer::new("Tokyo").unwrap(),
            response_modes,
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
    fn creates_content_with_one_mode_of_each_kind() {
        let content = new_content(
            Vec1::from_head_and_tail(free_input(), [text_choice()]),
            tags(10),
        )
        .unwrap();

        assert_eq!(content.response_modes().len().get(), 2);
        assert_eq!(content.tags().len(), 10);
    }

    #[test]
    fn rejects_duplicate_response_mode_kind() {
        let response_modes = Vec1::from_head_and_tail(free_input(), [text_choice(), free_input()]);

        assert_eq!(
            new_content(response_modes, tags(0)),
            Err(QuizContentError::DuplicateResponseMode)
        );
    }

    #[test]
    fn rejects_more_than_ten_tags() {
        assert_eq!(
            new_content(Vec1::from_one(free_input()), tags(11)),
            Err(QuizContentError::TooManyTags { count: 11 })
        );
    }
}
