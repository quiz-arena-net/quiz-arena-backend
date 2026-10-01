use std::{collections::BTreeSet, mem};

use isolang::Language;
use mitsein::{btree_set1::BTreeSet1, vec1::Vec1};
use uuid::Uuid;

use super::{
    media::{Media, MediaKind},
    tag::Tag,
    user::UserId,
};

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

/// An expected textual answer for a [`ResponseMode::FreeInput`] response.
///
/// Whether a submitted response is considered correct is determined by the
/// judging strategy configured for the session. For example, a session may use
/// exact matching, host judgment, or an LLM-based judge.
///
/// Guaranteed to be 1 to 200 characters, not only whitespace, and free of NUL.
/// Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FreeInputAnswer(String);

impl FreeInputAnswer {
    pub(crate) const MAX_LENGTH: usize = 200;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, FreeInputAnswerError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(FreeInputAnswerError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(FreeInputAnswerError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(FreeInputAnswerError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FreeInputAnswerError {
    #[error("free input answer must not be empty or only whitespace")]
    Blank,
    #[error(
        "free input answer must be at most {} characters, got {length}",
        FreeInputAnswer::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("free input answer must not contain NUL")]
    InvalidCharacter,
}

/// The player provides an unrestricted textual response.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FreeInput {
    /// Reference answers for judging.
    ///
    /// The actual judging strategy is configured separately by the session.
    expected_answers: BTreeSet1<FreeInputAnswer>,
}

impl FreeInput {
    pub(crate) fn new(expected_answers: BTreeSet1<FreeInputAnswer>) -> Self {
        Self { expected_answers }
    }

    pub(crate) fn expected_answers(&self) -> &BTreeSet1<FreeInputAnswer> {
        &self.expected_answers
    }
}

/// Specifies how many correct choices a player must select in a [`TextChoice`]
/// or [`ImageChoice`] response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ChoiceRequirement {
    /// Selecting any one correct answer is sufficient.
    One,

    /// Every correct answer must be selected.
    All,
}

/// An answer presented as text in a [`TextChoice`] response.
///
/// Guaranteed to be 1 to 200 characters, not only whitespace, and free of NUL.
/// Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct TextChoiceAnswer(String);

impl TextChoiceAnswer {
    pub(crate) const MAX_LENGTH: usize = 200;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, TextChoiceAnswerError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(TextChoiceAnswerError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(TextChoiceAnswerError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(TextChoiceAnswerError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TextChoiceAnswerError {
    #[error("text choice answer must not be empty or only whitespace")]
    Blank,
    #[error(
        "text choice answer must be at most {} characters, got {length}",
        TextChoiceAnswer::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("text choice answer must not contain NUL")]
    InvalidCharacter,
}

/// The player selects from a collection of predefined text answers.
///
/// Guaranteed to hold at most 6 answers, none of them both correct and wrong.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct TextChoice {
    /// Determines whether one or all correct answers must be selected.
    choice_requirement: ChoiceRequirement,

    /// Choices that are considered correct.
    correct_answers: BTreeSet1<TextChoiceAnswer>,

    /// Choices that are considered incorrect.
    ///
    /// This set may be empty, for example when every presented choice is
    /// intentionally correct.
    wrong_answers: BTreeSet<TextChoiceAnswer>,
}

impl TextChoice {
    pub(crate) const MAX_ANSWERS: usize = 6;

    pub(crate) fn new(
        choice_requirement: ChoiceRequirement,
        correct_answers: BTreeSet1<TextChoiceAnswer>,
        wrong_answers: BTreeSet<TextChoiceAnswer>,
    ) -> Result<Self, TextChoiceError> {
        if !correct_answers.is_disjoint(&wrong_answers) {
            return Err(TextChoiceError::AnswerBothCorrectAndWrong);
        }
        let count = correct_answers.len().get() + wrong_answers.len();
        if count > Self::MAX_ANSWERS {
            return Err(TextChoiceError::TooManyAnswers { count });
        }
        Ok(Self {
            choice_requirement,
            correct_answers,
            wrong_answers,
        })
    }

    pub(crate) fn choice_requirement(&self) -> ChoiceRequirement {
        self.choice_requirement
    }

    pub(crate) fn correct_answers(&self) -> &BTreeSet1<TextChoiceAnswer> {
        &self.correct_answers
    }

    pub(crate) fn wrong_answers(&self) -> &BTreeSet<TextChoiceAnswer> {
        &self.wrong_answers
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TextChoiceError {
    #[error("an answer must not be both correct and wrong")]
    AnswerBothCorrectAndWrong,
    #[error(
        "a text choice must have at most {} answers, got {count}",
        TextChoice::MAX_ANSWERS
    )]
    TooManyAnswers { count: usize },
}

/// Text shown alongside the image of an [`ImageChoiceAnswer`].
///
/// Guaranteed to be 1 to 200 characters, not only whitespace, and free of NUL.
/// Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ImageCaption(String);

impl ImageCaption {
    pub(crate) const MAX_LENGTH: usize = 200;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, ImageCaptionError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(ImageCaptionError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(ImageCaptionError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(ImageCaptionError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ImageCaptionError {
    #[error("image caption must not be empty or only whitespace")]
    Blank,
    #[error(
        "image caption must be at most {} characters, got {length}",
        ImageCaption::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("image caption must not contain NUL")]
    InvalidCharacter,
}

/// An answer presented as an image in an [`ImageChoice`] response.
///
/// Answers are told apart by their image.
///
/// Guaranteed to hold media of the image kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ImageChoiceAnswer {
    /// The image shown for this answer.
    image: Media,

    /// Text shown alongside the image.
    caption: Option<ImageCaption>,
}

impl ImageChoiceAnswer {
    pub(crate) fn new(
        image: Media,
        caption: Option<ImageCaption>,
    ) -> Result<Self, ImageChoiceAnswerError> {
        if image.kind() != MediaKind::Image {
            return Err(ImageChoiceAnswerError::NotAnImage);
        }
        Ok(Self { image, caption })
    }

    pub(crate) fn image(&self) -> Media {
        self.image
    }

    pub(crate) fn caption(&self) -> Option<&ImageCaption> {
        self.caption.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ImageChoiceAnswerError {
    #[error("an image choice answer must hold an image")]
    NotAnImage,
}

/// The player selects from a collection of predefined image answers.
///
/// Guaranteed to hold at most 4 answers, no two of them with the same image.
/// So no answer is both correct and wrong.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ImageChoice {
    /// Determines whether one or all correct answers must be selected.
    choice_requirement: ChoiceRequirement,

    /// Choices that are considered correct.
    correct_answers: BTreeSet1<ImageChoiceAnswer>,

    /// Choices that are considered incorrect.
    ///
    /// This set may be empty, for example when every presented choice is
    /// intentionally correct.
    wrong_answers: BTreeSet<ImageChoiceAnswer>,
}

impl ImageChoice {
    pub(crate) const MAX_ANSWERS: usize = 4;

    pub(crate) fn new(
        choice_requirement: ChoiceRequirement,
        correct_answers: BTreeSet1<ImageChoiceAnswer>,
        wrong_answers: BTreeSet<ImageChoiceAnswer>,
    ) -> Result<Self, ImageChoiceError> {
        let mut images = BTreeSet::new();
        for answer in wrong_answers.iter().chain(&correct_answers) {
            if !images.insert(answer.image.id()) {
                return Err(ImageChoiceError::DuplicateImage);
            }
        }
        let count = images.len();
        if count > Self::MAX_ANSWERS {
            return Err(ImageChoiceError::TooManyAnswers { count });
        }
        Ok(Self {
            choice_requirement,
            correct_answers,
            wrong_answers,
        })
    }

    pub(crate) fn choice_requirement(&self) -> ChoiceRequirement {
        self.choice_requirement
    }

    pub(crate) fn correct_answers(&self) -> &BTreeSet1<ImageChoiceAnswer> {
        &self.correct_answers
    }

    pub(crate) fn wrong_answers(&self) -> &BTreeSet<ImageChoiceAnswer> {
        &self.wrong_answers
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ImageChoiceError {
    #[error("an image must not appear in more than one answer")]
    DuplicateImage,
    #[error(
        "an image choice must have at most {} answers, got {count}",
        ImageChoice::MAX_ANSWERS
    )]
    TooManyAnswers { count: usize },
}

/// A single character used in a character-by-character response.
///
/// Guaranteed to be neither a control character nor whitespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Character(char);

impl Character {
    pub(crate) fn new(character: char) -> Result<Self, CharacterError> {
        if character.is_control() || character.is_whitespace() {
            return Err(CharacterError::Invalid);
        }
        Ok(Self(character))
    }

    pub(crate) fn as_char(&self) -> char {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CharacterError {
    #[error("character must not be a control character or whitespace")]
    Invalid,
}

/// The choices presented for one position in a character-by-character response.
///
/// Exactly one choice is correct. The remaining choices are distractors, and
/// none of them equals the correct one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CharacterChoice {
    /// The correct character for this position.
    correct_choice: Character,

    /// Incorrect characters that may be presented alongside the correct one.
    wrong_choices: BTreeSet1<Character>,
}

impl CharacterChoice {
    pub(crate) fn new(
        correct_choice: Character,
        wrong_choices: BTreeSet1<Character>,
    ) -> Result<Self, CharacterChoiceError> {
        if wrong_choices.contains(&correct_choice) {
            return Err(CharacterChoiceError::CorrectChoiceAmongWrongChoices);
        }
        Ok(Self {
            correct_choice,
            wrong_choices,
        })
    }

    pub(crate) fn correct_choice(&self) -> Character {
        self.correct_choice
    }

    pub(crate) fn wrong_choices(&self) -> &BTreeSet1<Character> {
        &self.wrong_choices
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CharacterChoiceError {
    #[error("the correct character must not also be a wrong choice")]
    CorrectChoiceAmongWrongChoices,
}

/// The player constructs the answer one character at a time by selecting from a
/// set of candidate characters at each position.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CharacterChoices {
    /// The choices for each position.
    ///
    /// The order is significant as it corresponds to the order of characters in
    /// the answer.
    choices: Vec1<CharacterChoice>,
}

impl CharacterChoices {
    pub(crate) fn new(choices: Vec1<CharacterChoice>) -> Self {
        Self { choices }
    }

    pub(crate) fn choices(&self) -> &Vec1<CharacterChoice> {
        &self.choices
    }
}

/// A way in which players may respond to a quiz.
///
/// A quiz may support multiple response modes. The game or session hosting the
/// quiz can restrict which of those modes are used.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ResponseMode {
    FreeInput(FreeInput),
    TextChoice(TextChoice),
    ImageChoice(ImageChoice),
    CharacterChoices(CharacterChoices),
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
    use super::*;
    use crate::domain::media::MediaId;

    fn character(character: char) -> Character {
        Character::new(character).unwrap()
    }

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
        assert!(FreeInputAnswer::new("x".repeat(200)).is_ok());
        assert!(TextChoiceAnswer::new("x".repeat(200)).is_ok());
        assert!(ImageCaption::new("x".repeat(200)).is_ok());
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
        assert_eq!(
            FreeInputAnswer::new("x".repeat(201)),
            Err(FreeInputAnswerError::TooLong { length: 201 })
        );
        assert_eq!(
            TextChoiceAnswer::new("x".repeat(201)),
            Err(TextChoiceAnswerError::TooLong { length: 201 })
        );
        assert_eq!(
            ImageCaption::new("x".repeat(201)),
            Err(ImageCaptionError::TooLong { length: 201 })
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
            assert_eq!(
                FreeInputAnswer::new(text),
                Err(FreeInputAnswerError::Blank),
                "{text:?}"
            );
            assert_eq!(
                TextChoiceAnswer::new(text),
                Err(TextChoiceAnswerError::Blank),
                "{text:?}"
            );
            assert_eq!(
                ImageCaption::new(text),
                Err(ImageCaptionError::Blank),
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
            assert_eq!(
                FreeInputAnswer::new(text),
                Err(FreeInputAnswerError::InvalidCharacter),
                "{text:?}"
            );
            assert_eq!(
                TextChoiceAnswer::new(text),
                Err(TextChoiceAnswerError::InvalidCharacter),
                "{text:?}"
            );
            assert_eq!(
                ImageCaption::new(text),
                Err(ImageCaptionError::InvalidCharacter),
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
    fn accepts_printable_characters() {
        for character in ['a', 'あ', '1', '!', '😀'] {
            assert_eq!(Character::new(character).unwrap().as_char(), character);
        }
    }

    #[test]
    fn rejects_control_and_whitespace_characters() {
        for character in ['\0', '\t', '\n', '\u{7f}', ' ', '\u{a0}', '\u{3000}'] {
            assert_eq!(
                Character::new(character),
                Err(CharacterError::Invalid),
                "{character:?}"
            );
        }
    }

    #[test]
    fn rejects_correct_character_among_wrong_choices() {
        let wrong_choices = BTreeSet1::from_head_and_tail(character('a'), [character('b')]);

        assert_eq!(
            CharacterChoice::new(character('a'), wrong_choices),
            Err(CharacterChoiceError::CorrectChoiceAmongWrongChoices)
        );
    }

    #[test]
    fn accepts_distinct_character_choices() {
        let choice =
            CharacterChoice::new(character('a'), BTreeSet1::from_one(character('b'))).unwrap();

        assert_eq!(choice.correct_choice(), character('a'));
    }

    #[test]
    fn rejects_answer_both_correct_and_wrong() {
        let tokyo = TextChoiceAnswer::new("Tokyo").unwrap();

        assert_eq!(
            TextChoice::new(
                ChoiceRequirement::All,
                BTreeSet1::from_one(tokyo.clone()),
                BTreeSet::from([tokyo]),
            ),
            Err(TextChoiceError::AnswerBothCorrectAndWrong)
        );
    }

    #[test]
    fn accepts_text_choice_without_wrong_answers() {
        assert!(
            TextChoice::new(
                ChoiceRequirement::All,
                BTreeSet1::from_one(TextChoiceAnswer::new("Tokyo").unwrap()),
                BTreeSet::new(),
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_more_than_six_text_choice_answers() {
        let text_choice = |wrong_count: usize| {
            TextChoice::new(
                ChoiceRequirement::One,
                BTreeSet1::from_one(TextChoiceAnswer::new("correct").unwrap()),
                (0..wrong_count)
                    .map(|index| TextChoiceAnswer::new(format!("wrong{index}")).unwrap())
                    .collect(),
            )
        };

        assert!(text_choice(5).is_ok());
        assert_eq!(
            text_choice(6),
            Err(TextChoiceError::TooManyAnswers { count: 7 })
        );
    }

    fn media(kind: MediaKind) -> Media {
        Media::new(MediaId::from_uuid(Uuid::now_v7()), kind)
    }

    fn image_answer() -> ImageChoiceAnswer {
        ImageChoiceAnswer::new(media(MediaKind::Image), None).unwrap()
    }

    #[test]
    fn rejects_image_choice_answer_without_an_image() {
        for kind in [MediaKind::Audio, MediaKind::Video] {
            assert_eq!(
                ImageChoiceAnswer::new(media(kind), None),
                Err(ImageChoiceAnswerError::NotAnImage),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn rejects_more_than_four_image_choice_answers() {
        let image_choice = |wrong_count: usize| {
            ImageChoice::new(
                ChoiceRequirement::One,
                BTreeSet1::from_one(image_answer()),
                (0..wrong_count).map(|_| image_answer()).collect(),
            )
        };

        assert!(image_choice(3).is_ok());
        assert_eq!(
            image_choice(4),
            Err(ImageChoiceError::TooManyAnswers { count: 5 })
        );
    }

    #[test]
    fn rejects_the_same_image_in_two_answers() {
        let image = media(MediaKind::Image);
        let plain = ImageChoiceAnswer::new(image, None).unwrap();
        let labelled =
            ImageChoiceAnswer::new(image, Some(ImageCaption::new("Tokyo").unwrap())).unwrap();

        assert_eq!(
            ImageChoice::new(
                ChoiceRequirement::All,
                BTreeSet1::from_head_and_tail(plain.clone(), [labelled]),
                BTreeSet::new(),
            ),
            Err(ImageChoiceError::DuplicateImage)
        );
        assert_eq!(
            ImageChoice::new(
                ChoiceRequirement::One,
                BTreeSet1::from_one(plain.clone()),
                BTreeSet::from([plain]),
            ),
            Err(ImageChoiceError::DuplicateImage)
        );
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
