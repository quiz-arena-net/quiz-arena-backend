use std::{collections::BTreeSet, mem};

use isolang::Language;
use mitsein::vec1::Vec1;
use uuid::Uuid;

use super::{media::Media, response_mode::ResponseMode, tag::Tag, user::UserId};

/// Uniquely identifies a quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizId(Uuid);

impl QuizId {
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

/// A quiz that can be presented to players.
///
/// A quiz defines its authored content and the response modes it supports.
/// Session-specific behavior, such as how free-input responses are judged, is
/// configured outside the aggregate.
///
/// Guaranteed to support at most one response mode of each kind and to carry at
/// most 10 tags. Whether a caller may edit the quiz is decided by the
/// application layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Quiz {
    /// The unique identifier of this quiz.
    id: QuizId,

    /// The author who created this quiz.
    author: UserId,

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

impl Quiz {
    pub(crate) const MAX_TAGS: usize = 10;

    fn check_response_modes(response_modes: &Vec1<ResponseMode>) -> Result<(), QuizError> {
        let mut seen_kinds = Vec::new();
        for response_mode in response_modes.iter() {
            let kind = mem::discriminant(response_mode);
            if seen_kinds.contains(&kind) {
                return Err(QuizError::DuplicateResponseMode);
            }
            seen_kinds.push(kind);
        }
        Ok(())
    }

    fn check_tags(tags: &BTreeSet<Tag>) -> Result<(), QuizError> {
        let count = tags.len();
        if count > Self::MAX_TAGS {
            return Err(QuizError::TooManyTags { count });
        }
        Ok(())
    }

    /// Creates a new quiz with a freshly generated identifier.
    pub(crate) fn new(
        author: UserId,
        prompt: Prompt,
        canonical_answer: CanonicalAnswer,
        response_modes: Vec1<ResponseMode>,
        language: Language,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, QuizError> {
        Self::check_response_modes(&response_modes)?;
        Self::check_tags(&tags)?;
        Ok(Self {
            id: QuizId::new(),
            author,
            prompt,
            canonical_answer,
            response_modes,
            language,
            tags,
        })
    }

    /// Rehydrates a quiz from persisted state.
    ///
    /// For repository implementations only. Checks the aggregate invariants,
    /// which hold for every quiz however it was obtained. Restrictions that
    /// apply only when creating a quiz are not rechecked here.
    pub(crate) fn from_persistence(
        id: QuizId,
        author: UserId,
        prompt: Prompt,
        canonical_answer: CanonicalAnswer,
        response_modes: Vec1<ResponseMode>,
        language: Language,
        tags: BTreeSet<Tag>,
    ) -> Result<Self, QuizError> {
        Self::check_response_modes(&response_modes)?;
        Self::check_tags(&tags)?;
        Ok(Self {
            id,
            author,
            prompt,
            canonical_answer,
            response_modes,
            language,
            tags,
        })
    }

    pub(crate) fn id(&self) -> QuizId {
        self.id
    }

    pub(crate) fn author(&self) -> UserId {
        self.author
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

    pub(crate) fn change_prompt(&mut self, prompt: Prompt) {
        self.prompt = prompt;
    }

    pub(crate) fn change_canonical_answer(&mut self, canonical_answer: CanonicalAnswer) {
        self.canonical_answer = canonical_answer;
    }

    /// Replaces the supported response modes. Fails only with
    /// [`QuizError::DuplicateResponseMode`].
    pub(crate) fn change_response_modes(
        &mut self,
        response_modes: Vec1<ResponseMode>,
    ) -> Result<(), QuizError> {
        Self::check_response_modes(&response_modes)?;
        self.response_modes = response_modes;
        Ok(())
    }

    pub(crate) fn change_language(&mut self, language: Language) {
        self.language = language;
    }

    /// Replaces the tags. Fails only with [`QuizError::TooManyTags`].
    pub(crate) fn change_tags(&mut self, tags: BTreeSet<Tag>) -> Result<(), QuizError> {
        Self::check_tags(&tags)?;
        self.tags = tags;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum QuizError {
    #[error("a quiz must not support the same kind of response mode twice")]
    DuplicateResponseMode,
    #[error("a quiz must have at most {} tags, got {count}", Quiz::MAX_TAGS)]
    TooManyTags { count: usize },
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

    fn new_quiz(
        response_modes: Vec1<ResponseMode>,
        tags: BTreeSet<Tag>,
    ) -> Result<Quiz, QuizError> {
        Quiz::new(
            UserId::from_uuid(Uuid::now_v7()),
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
    fn creates_quiz_with_one_mode_of_each_kind() {
        let quiz = new_quiz(
            Vec1::from_head_and_tail(free_input(), [text_choice()]),
            tags(10),
        )
        .unwrap();

        assert_eq!(quiz.response_modes().len().get(), 2);
        assert_eq!(quiz.tags().len(), 10);
    }

    #[test]
    fn rejects_duplicate_response_mode_kind() {
        let response_modes = Vec1::from_head_and_tail(free_input(), [text_choice(), free_input()]);

        assert_eq!(
            new_quiz(response_modes, tags(0)),
            Err(QuizError::DuplicateResponseMode)
        );
    }

    #[test]
    fn rejects_more_than_ten_tags() {
        assert_eq!(
            new_quiz(Vec1::from_one(free_input()), tags(11)),
            Err(QuizError::TooManyTags { count: 11 })
        );
    }

    #[test]
    fn new_quizzes_get_distinct_ids() {
        let first = new_quiz(Vec1::from_one(free_input()), tags(0)).unwrap();
        let second = new_quiz(Vec1::from_one(free_input()), tags(0)).unwrap();

        assert_ne!(first.id(), second.id());
    }

    #[test]
    fn rehydration_checks_invariants() {
        let rehydrate = |response_modes, tags| {
            Quiz::from_persistence(
                QuizId::new(),
                UserId::from_uuid(Uuid::now_v7()),
                Prompt::new(
                    PromptText::new("What is the capital of Japan?").unwrap(),
                    None,
                ),
                CanonicalAnswer::new("Tokyo").unwrap(),
                response_modes,
                Language::Eng,
                tags,
            )
        };

        assert!(rehydrate(Vec1::from_one(free_input()), tags(10)).is_ok());
        assert_eq!(
            rehydrate(
                Vec1::from_head_and_tail(free_input(), [free_input()]),
                tags(0)
            ),
            Err(QuizError::DuplicateResponseMode)
        );
        assert_eq!(
            rehydrate(Vec1::from_one(free_input()), tags(11)),
            Err(QuizError::TooManyTags { count: 11 })
        );
    }

    #[test]
    fn rejected_change_leaves_quiz_unchanged() {
        let mut quiz = new_quiz(Vec1::from_one(free_input()), tags(1)).unwrap();
        let before = quiz.clone();

        assert_eq!(
            quiz.change_response_modes(Vec1::from_head_and_tail(free_input(), [free_input()])),
            Err(QuizError::DuplicateResponseMode)
        );
        assert_eq!(
            quiz.change_tags(tags(11)),
            Err(QuizError::TooManyTags { count: 11 })
        );
        assert_eq!(quiz, before);
    }

    #[test]
    fn accepted_change_replaces_content() {
        let mut quiz = new_quiz(Vec1::from_one(free_input()), tags(1)).unwrap();

        quiz.change_prompt(Prompt::new(
            PromptText::new("Capital of Japan?").unwrap(),
            None,
        ));
        quiz.change_response_modes(Vec1::from_one(text_choice()))
            .unwrap();
        quiz.change_tags(tags(3)).unwrap();

        assert_eq!(quiz.prompt().text().as_str(), "Capital of Japan?");
        assert_eq!(quiz.response_modes().as_slice(), [text_choice()]);
        assert_eq!(quiz.tags().len(), 3);
    }
}
