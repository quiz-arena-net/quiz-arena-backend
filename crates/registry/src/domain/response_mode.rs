use std::collections::BTreeSet;

use mitsein::{btree_set1::BTreeSet1, vec1::Vec1};
use unicode_normalization::UnicodeNormalization;

use super::media::{Media, MediaKind};

/// An expected textual answer for a [`ResponseMode::FreeInput`] response.
///
/// Whether a submitted response is considered correct is determined by the
/// judging strategy configured for the session. For example, a session may use
/// exact matching, host judgment, or an LLM-based judge.
///
/// Guaranteed to be 1 to 200 characters in NFC, with no surrounding whitespace
/// and no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FreeInputAnswer(String);

impl FreeInputAnswer {
    pub(crate) const MAX_LENGTH: usize = 200;

    /// Parses text into a free input answer.
    ///
    /// Trims surrounding whitespace and normalizes the input text to NFC.
    pub(crate) fn parse(text: impl Into<String>) -> Result<Self, FreeInputAnswerParseError> {
        let text: String = text.into().trim().nfc().collect();
        if text.is_empty() {
            return Err(FreeInputAnswerParseError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(FreeInputAnswerParseError::TooLong { length });
        }
        if text.chars().any(char::is_control) {
            return Err(FreeInputAnswerParseError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FreeInputAnswerParseError {
    #[error("free input answer must not be empty or only whitespace")]
    Blank,
    #[error(
        "free input answer must be at most {} characters, got {length}",
        FreeInputAnswer::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("free input answer must not contain control characters")]
    InvalidCharacter,
}

/// A [`FreeInput`] that may still be incomplete.
///
/// The expected answers may be empty and their limit is not enforced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftFreeInput {
    /// Reference answers for judging.
    expected_answers: BTreeSet<FreeInputAnswer>,
}

impl DraftFreeInput {
    pub(crate) fn new(expected_answers: BTreeSet<FreeInputAnswer>) -> Self {
        Self { expected_answers }
    }

    pub(crate) fn expected_answers(&self) -> &BTreeSet<FreeInputAnswer> {
        &self.expected_answers
    }
}

/// The player provides an unrestricted textual response.
///
/// Guaranteed to hold 1 to 20 expected answers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FreeInput {
    /// Reference answers for judging.
    ///
    /// The actual judging strategy is configured separately by the session.
    expected_answers: BTreeSet1<FreeInputAnswer>,
}

impl FreeInput {
    pub(crate) const MAX_EXPECTED_ANSWERS: usize = 20;

    pub(crate) fn new(
        expected_answers: BTreeSet1<FreeInputAnswer>,
    ) -> Result<Self, FreeInputError> {
        let count = expected_answers.len().get();
        if count > Self::MAX_EXPECTED_ANSWERS {
            return Err(FreeInputError::TooManyExpectedAnswers { count });
        }
        Ok(Self { expected_answers })
    }

    pub(crate) fn expected_answers(&self) -> &BTreeSet1<FreeInputAnswer> {
        &self.expected_answers
    }
}

impl TryFrom<DraftFreeInput> for FreeInput {
    type Error = FreeInputFromDraftError;

    fn try_from(draft: DraftFreeInput) -> Result<Self, Self::Error> {
        let expected_answers = match BTreeSet1::try_from(draft.expected_answers) {
            Ok(expected_answers) => expected_answers,
            Err(_) => return Err(FreeInputFromDraftError::NoExpectedAnswer),
        };
        match Self::new(expected_answers) {
            Ok(free_input) => Ok(free_input),
            Err(FreeInputError::TooManyExpectedAnswers { count }) => {
                Err(FreeInputFromDraftError::TooManyExpectedAnswers { count })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FreeInputError {
    #[error(
        "a free input must have at most {} expected answers, got {count}",
        FreeInput::MAX_EXPECTED_ANSWERS
    )]
    TooManyExpectedAnswers { count: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FreeInputFromDraftError {
    #[error("a free input must have at least one expected answer")]
    NoExpectedAnswer,
    #[error(
        "a free input must have at most {} expected answers, got {count}",
        FreeInput::MAX_EXPECTED_ANSWERS
    )]
    TooManyExpectedAnswers { count: usize },
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
/// Guaranteed to be 1 to 200 characters in NFC, with no surrounding
/// whitespace and no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct TextChoiceAnswer(String);

impl TextChoiceAnswer {
    pub(crate) const MAX_LENGTH: usize = 200;

    /// Parses text into a text choice answer.
    ///
    /// Trims surrounding whitespace and normalizes the input text to NFC.
    pub(crate) fn parse(text: impl Into<String>) -> Result<Self, TextChoiceAnswerParseError> {
        let text: String = text.into().trim().nfc().collect();
        if text.is_empty() {
            return Err(TextChoiceAnswerParseError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(TextChoiceAnswerParseError::TooLong { length });
        }
        if text.chars().any(char::is_control) {
            return Err(TextChoiceAnswerParseError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TextChoiceAnswerParseError {
    #[error("text choice answer must not be empty or only whitespace")]
    Blank,
    #[error(
        "text choice answer must be at most {} characters, got {length}",
        TextChoiceAnswer::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("text choice answer must not contain control characters")]
    InvalidCharacter,
}

/// A [`TextChoice`] that may still be incomplete.
///
/// The correct answers may be empty, an answer may be both correct and wrong,
/// and the answer limit is not enforced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftTextChoice {
    /// Determines whether one or all correct answers must be selected.
    choice_requirement: ChoiceRequirement,

    /// Choices that are considered correct.
    correct_answers: BTreeSet<TextChoiceAnswer>,

    /// Choices that are considered incorrect.
    wrong_answers: BTreeSet<TextChoiceAnswer>,
}

impl DraftTextChoice {
    pub(crate) fn new(
        choice_requirement: ChoiceRequirement,
        correct_answers: BTreeSet<TextChoiceAnswer>,
        wrong_answers: BTreeSet<TextChoiceAnswer>,
    ) -> Self {
        Self {
            choice_requirement,
            correct_answers,
            wrong_answers,
        }
    }

    pub(crate) fn choice_requirement(&self) -> ChoiceRequirement {
        self.choice_requirement
    }

    pub(crate) fn correct_answers(&self) -> &BTreeSet<TextChoiceAnswer> {
        &self.correct_answers
    }

    pub(crate) fn wrong_answers(&self) -> &BTreeSet<TextChoiceAnswer> {
        &self.wrong_answers
    }
}

/// The player selects from a collection of predefined text answers.
///
/// Guaranteed to hold 1 to 6 answers with at least one correct, none of them
/// both correct and wrong.
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

impl TryFrom<DraftTextChoice> for TextChoice {
    type Error = TextChoiceFromDraftError;

    fn try_from(draft: DraftTextChoice) -> Result<Self, Self::Error> {
        let correct_answers = match BTreeSet1::try_from(draft.correct_answers) {
            Ok(correct_answers) => correct_answers,
            Err(_) => return Err(TextChoiceFromDraftError::NoCorrectAnswer),
        };
        match Self::new(
            draft.choice_requirement,
            correct_answers,
            draft.wrong_answers,
        ) {
            Ok(text_choice) => Ok(text_choice),
            Err(TextChoiceError::AnswerBothCorrectAndWrong) => {
                Err(TextChoiceFromDraftError::AnswerBothCorrectAndWrong)
            }
            Err(TextChoiceError::TooManyAnswers { count }) => {
                Err(TextChoiceFromDraftError::TooManyAnswers { count })
            }
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TextChoiceFromDraftError {
    #[error("a text choice must have at least one correct answer")]
    NoCorrectAnswer,
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
/// Guaranteed to be 1 to 200 characters in NFC, with no surrounding whitespace
/// and no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ImageCaption(String);

impl ImageCaption {
    pub(crate) const MAX_LENGTH: usize = 200;

    /// Parses text into an image caption.
    ///
    /// Trims surrounding whitespace and normalizes the input text to NFC.
    pub(crate) fn parse(text: impl Into<String>) -> Result<Self, ImageCaptionParseError> {
        let text: String = text.into().trim().nfc().collect();
        if text.is_empty() {
            return Err(ImageCaptionParseError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(ImageCaptionParseError::TooLong { length });
        }
        if text.chars().any(char::is_control) {
            return Err(ImageCaptionParseError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ImageCaptionParseError {
    #[error("image caption must not be empty or only whitespace")]
    Blank,
    #[error(
        "image caption must be at most {} characters, got {length}",
        ImageCaption::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("image caption must not contain control characters")]
    InvalidCharacter,
}

/// An answer presented as an image in an [`ImageChoice`] response.
///
/// Guaranteed to hold media of the image kind. Within an [`ImageChoice`] no two
/// answers share an image.
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

/// An [`ImageChoice`] that may still be incomplete.
///
/// The correct answers may be empty, an image may appear twice, and the answer
/// limit is not enforced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftImageChoice {
    /// Determines whether one or all correct answers must be selected.
    choice_requirement: ChoiceRequirement,

    /// Choices that are considered correct.
    correct_answers: BTreeSet<ImageChoiceAnswer>,

    /// Choices that are considered incorrect.
    wrong_answers: BTreeSet<ImageChoiceAnswer>,
}

impl DraftImageChoice {
    pub(crate) fn new(
        choice_requirement: ChoiceRequirement,
        correct_answers: BTreeSet<ImageChoiceAnswer>,
        wrong_answers: BTreeSet<ImageChoiceAnswer>,
    ) -> Self {
        Self {
            choice_requirement,
            correct_answers,
            wrong_answers,
        }
    }

    pub(crate) fn choice_requirement(&self) -> ChoiceRequirement {
        self.choice_requirement
    }

    pub(crate) fn correct_answers(&self) -> &BTreeSet<ImageChoiceAnswer> {
        &self.correct_answers
    }

    pub(crate) fn wrong_answers(&self) -> &BTreeSet<ImageChoiceAnswer> {
        &self.wrong_answers
    }
}

/// The player selects from a collection of predefined image answers.
///
/// Guaranteed to hold 1 to 4 answers with at least one correct, no two of them
/// with the same image, which also keeps every answer from being both correct
/// and wrong.
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
        for answer in wrong_answers.iter().chain(correct_answers.as_btree_set()) {
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

impl TryFrom<DraftImageChoice> for ImageChoice {
    type Error = ImageChoiceFromDraftError;

    fn try_from(draft: DraftImageChoice) -> Result<Self, Self::Error> {
        let correct_answers = match BTreeSet1::try_from(draft.correct_answers) {
            Ok(correct_answers) => correct_answers,
            Err(_) => return Err(ImageChoiceFromDraftError::NoCorrectAnswer),
        };
        match Self::new(
            draft.choice_requirement,
            correct_answers,
            draft.wrong_answers,
        ) {
            Ok(image_choice) => Ok(image_choice),
            Err(ImageChoiceError::DuplicateImage) => Err(ImageChoiceFromDraftError::DuplicateImage),
            Err(ImageChoiceError::TooManyAnswers { count }) => {
                Err(ImageChoiceFromDraftError::TooManyAnswers { count })
            }
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ImageChoiceFromDraftError {
    #[error("an image choice must have at least one correct answer")]
    NoCorrectAnswer,
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

/// A [`CharacterChoice`] that may still be incomplete.
///
/// The correct choice may be missing, the wrong choices may be empty, the
/// correct choice may also appear among the wrong ones, and the limit on wrong
/// choices is not enforced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftCharacterChoice {
    /// The correct character for this position.
    correct_choice: Option<Character>,

    /// Incorrect characters that may be presented alongside the correct one.
    wrong_choices: BTreeSet<Character>,
}

impl DraftCharacterChoice {
    pub(crate) fn new(
        correct_choice: Option<Character>,
        wrong_choices: BTreeSet<Character>,
    ) -> Self {
        Self {
            correct_choice,
            wrong_choices,
        }
    }

    pub(crate) fn correct_choice(&self) -> Option<Character> {
        self.correct_choice
    }

    pub(crate) fn wrong_choices(&self) -> &BTreeSet<Character> {
        &self.wrong_choices
    }
}

/// The choices presented for one position in a character-by-character response.
///
/// Exactly one choice is correct. The remaining choices are distractors, and
/// none of them equals the correct one. Guaranteed to hold 1 to 9 wrong
/// choices.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CharacterChoice {
    /// The correct character for this position.
    correct_choice: Character,

    /// Incorrect characters that may be presented alongside the correct one.
    wrong_choices: BTreeSet1<Character>,
}

impl CharacterChoice {
    pub(crate) const MAX_WRONG_CHOICES: usize = 9;

    pub(crate) fn new(
        correct_choice: Character,
        wrong_choices: BTreeSet1<Character>,
    ) -> Result<Self, CharacterChoiceError> {
        if wrong_choices.contains(&correct_choice) {
            return Err(CharacterChoiceError::CorrectChoiceAmongWrongChoices);
        }
        let count = wrong_choices.len().get();
        if count > Self::MAX_WRONG_CHOICES {
            return Err(CharacterChoiceError::TooManyWrongChoices { count });
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

impl TryFrom<DraftCharacterChoice> for CharacterChoice {
    type Error = CharacterChoiceFromDraftError;

    fn try_from(draft: DraftCharacterChoice) -> Result<Self, Self::Error> {
        let Some(correct_choice) = draft.correct_choice else {
            return Err(CharacterChoiceFromDraftError::NoCorrectChoice);
        };
        let wrong_choices = match BTreeSet1::try_from(draft.wrong_choices) {
            Ok(wrong_choices) => wrong_choices,
            Err(_) => return Err(CharacterChoiceFromDraftError::NoWrongChoice),
        };
        match Self::new(correct_choice, wrong_choices) {
            Ok(choice) => Ok(choice),
            Err(CharacterChoiceError::CorrectChoiceAmongWrongChoices) => {
                Err(CharacterChoiceFromDraftError::CorrectChoiceAmongWrongChoices)
            }
            Err(CharacterChoiceError::TooManyWrongChoices { count }) => {
                Err(CharacterChoiceFromDraftError::TooManyWrongChoices { count })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CharacterChoiceError {
    #[error("the correct character must not also be a wrong choice")]
    CorrectChoiceAmongWrongChoices,
    #[error(
        "a character choice must have at most {} wrong choices, got {count}",
        CharacterChoice::MAX_WRONG_CHOICES
    )]
    TooManyWrongChoices { count: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CharacterChoiceFromDraftError {
    #[error("a character choice must have a correct choice")]
    NoCorrectChoice,
    #[error("a character choice must have at least one wrong choice")]
    NoWrongChoice,
    #[error("the correct character must not also be a wrong choice")]
    CorrectChoiceAmongWrongChoices,
    #[error(
        "a character choice must have at most {} wrong choices, got {count}",
        CharacterChoice::MAX_WRONG_CHOICES
    )]
    TooManyWrongChoices { count: usize },
}

/// A [`CharacterChoices`] that may still be incomplete.
///
/// The positions may be empty and their limit is not enforced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftCharacterChoices {
    /// The choices for each position.
    choices: Vec<DraftCharacterChoice>,
}

impl DraftCharacterChoices {
    pub(crate) fn new(choices: Vec<DraftCharacterChoice>) -> Self {
        Self { choices }
    }

    pub(crate) fn choices(&self) -> &[DraftCharacterChoice] {
        &self.choices
    }
}

/// The player constructs the answer one character at a time by selecting from a
/// set of candidate characters at each position.
///
/// Guaranteed to hold 1 to 50 positions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CharacterChoices {
    /// The choices for each position.
    ///
    /// The order is significant as it corresponds to the order of characters in
    /// the answer.
    choices: Vec1<CharacterChoice>,
}

impl CharacterChoices {
    pub(crate) const MAX_POSITIONS: usize = 50;

    pub(crate) fn new(choices: Vec1<CharacterChoice>) -> Result<Self, CharacterChoicesError> {
        let count = choices.len().get();
        if count > Self::MAX_POSITIONS {
            return Err(CharacterChoicesError::TooManyCharacterChoices { count });
        }
        Ok(Self { choices })
    }

    pub(crate) fn choices(&self) -> &Vec1<CharacterChoice> {
        &self.choices
    }
}

impl TryFrom<DraftCharacterChoices> for CharacterChoices {
    type Error = CharacterChoicesFromDraftError;

    /// Stops at the first position that cannot be converted.
    fn try_from(draft: DraftCharacterChoices) -> Result<Self, Self::Error> {
        let mut choices = Vec::with_capacity(draft.choices.len());
        for (position, choice) in draft.choices.into_iter().enumerate() {
            match CharacterChoice::try_from(choice) {
                Ok(choice) => choices.push(choice),
                Err(error) => {
                    return Err(CharacterChoicesFromDraftError::InvalidCharacterChoice {
                        position,
                        error,
                    });
                }
            }
        }
        let choices = match Vec1::try_from(choices) {
            Ok(choices) => choices,
            Err(_) => return Err(CharacterChoicesFromDraftError::NoCharacterChoice),
        };
        match Self::new(choices) {
            Ok(choices) => Ok(choices),
            Err(CharacterChoicesError::TooManyCharacterChoices { count }) => {
                Err(CharacterChoicesFromDraftError::TooManyCharacterChoices { count })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CharacterChoicesError {
    #[error(
        "character choices must have at most {} positions, got {count}",
        CharacterChoices::MAX_POSITIONS
    )]
    TooManyCharacterChoices { count: usize },
}

/// Positions are zero-based indexes into [`DraftCharacterChoices::choices`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CharacterChoicesFromDraftError {
    #[error("character choices must have at least one position")]
    NoCharacterChoice,
    #[error("the character choice at position {position} is invalid")]
    InvalidCharacterChoice {
        position: usize,
        error: CharacterChoiceFromDraftError,
    },
    #[error(
        "character choices must have at most {} positions, got {count}",
        CharacterChoices::MAX_POSITIONS
    )]
    TooManyCharacterChoices { count: usize },
}

/// A [`ResponseMode`] that may still be incomplete.
///
/// Each variant relaxes the guarantees of its counterpart. Individual answers
/// are the parsed value types, so a draft holds either a complete answer or
/// none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DraftResponseMode {
    FreeInput(DraftFreeInput),
    TextChoice(DraftTextChoice),
    ImageChoice(DraftImageChoice),
    CharacterChoices(DraftCharacterChoices),
}

/// The way in which players respond to a quiz.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ResponseMode {
    FreeInput(FreeInput),
    TextChoice(TextChoice),
    ImageChoice(ImageChoice),
    CharacterChoices(CharacterChoices),
}

impl TryFrom<DraftResponseMode> for ResponseMode {
    type Error = ResponseModeFromDraftError;

    fn try_from(draft: DraftResponseMode) -> Result<Self, Self::Error> {
        match draft {
            DraftResponseMode::FreeInput(free_input) => FreeInput::try_from(free_input)
                .map(Self::FreeInput)
                .map_err(ResponseModeFromDraftError::FreeInput),
            DraftResponseMode::TextChoice(text_choice) => TextChoice::try_from(text_choice)
                .map(Self::TextChoice)
                .map_err(ResponseModeFromDraftError::TextChoice),
            DraftResponseMode::ImageChoice(image_choice) => ImageChoice::try_from(image_choice)
                .map(Self::ImageChoice)
                .map_err(ResponseModeFromDraftError::ImageChoice),
            DraftResponseMode::CharacterChoices(character_choices) => {
                CharacterChoices::try_from(character_choices)
                    .map(Self::CharacterChoices)
                    .map_err(ResponseModeFromDraftError::CharacterChoices)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ResponseModeFromDraftError {
    #[error("the free input is invalid")]
    FreeInput(FreeInputFromDraftError),
    #[error("the text choice is invalid")]
    TextChoice(TextChoiceFromDraftError),
    #[error("the image choice is invalid")]
    ImageChoice(ImageChoiceFromDraftError),
    #[error("the character choices are invalid")]
    CharacterChoices(CharacterChoicesFromDraftError),
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::media::MediaId;

    fn character(character: char) -> Character {
        Character::new(character).unwrap()
    }

    #[test]
    fn accepts_text_within_bounds() {
        assert!(FreeInputAnswer::parse("x".repeat(200)).is_ok());
        assert!(TextChoiceAnswer::parse("x".repeat(200)).is_ok());
        assert!(ImageCaption::parse("x".repeat(200)).is_ok());
    }

    #[test]
    fn rejects_text_over_the_limit() {
        assert_eq!(
            FreeInputAnswer::parse("x".repeat(201)),
            Err(FreeInputAnswerParseError::TooLong { length: 201 })
        );
        assert_eq!(
            TextChoiceAnswer::parse("x".repeat(201)),
            Err(TextChoiceAnswerParseError::TooLong { length: 201 })
        );
        assert_eq!(
            ImageCaption::parse("x".repeat(201)),
            Err(ImageCaptionParseError::TooLong { length: 201 })
        );
    }

    #[test]
    fn rejects_blank_text() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
            assert_eq!(
                FreeInputAnswer::parse(text),
                Err(FreeInputAnswerParseError::Blank),
                "{text:?}"
            );
            assert_eq!(
                TextChoiceAnswer::parse(text),
                Err(TextChoiceAnswerParseError::Blank),
                "{text:?}"
            );
            assert_eq!(
                ImageCaption::parse(text),
                Err(ImageCaptionParseError::Blank),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_control_characters() {
        for text in ["\0", "a\0b", "a\rb", "a\nb", "a\tb", "a\u{7f}b", "a\u{85}b"] {
            assert_eq!(
                FreeInputAnswer::parse(text),
                Err(FreeInputAnswerParseError::InvalidCharacter),
                "{text:?}"
            );
            assert_eq!(
                TextChoiceAnswer::parse(text),
                Err(TextChoiceAnswerParseError::InvalidCharacter),
                "{text:?}"
            );
            assert_eq!(
                ImageCaption::parse(text),
                Err(ImageCaptionParseError::InvalidCharacter),
                "{text:?}"
            );
        }
    }

    #[test]
    fn trims_surrounding_whitespace() {
        for text in [" Tokyo ", "\tTokyo\n", "\u{3000}Tokyo"] {
            assert_eq!(
                FreeInputAnswer::parse(text).unwrap().as_str(),
                "Tokyo",
                "{text:?}"
            );
            assert_eq!(
                TextChoiceAnswer::parse(text).unwrap().as_str(),
                "Tokyo",
                "{text:?}"
            );
            assert_eq!(
                ImageCaption::parse(text).unwrap().as_str(),
                "Tokyo",
                "{text:?}"
            );
        }
    }

    #[test]
    fn normalizes_to_nfc() {
        assert_eq!(
            FreeInputAnswer::parse("a\u{301}").unwrap().as_str(),
            "\u{e1}"
        );
        assert_eq!(FreeInputAnswer::parse("か\u{3099}").unwrap().as_str(), "が");
        assert_eq!(
            TextChoiceAnswer::parse("a\u{301}").unwrap().as_str(),
            "\u{e1}"
        );
        assert_eq!(
            TextChoiceAnswer::parse("か\u{3099}").unwrap().as_str(),
            "が"
        );
        assert_eq!(ImageCaption::parse("a\u{301}").unwrap().as_str(), "\u{e1}");
        assert_eq!(ImageCaption::parse("か\u{3099}").unwrap().as_str(), "が");
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
    fn rejects_more_than_twenty_expected_answers() {
        let free_input = |count: usize| {
            FreeInput::new(BTreeSet1::from_head_and_tail(
                FreeInputAnswer::parse("answer0").unwrap(),
                (1..count).map(|index| FreeInputAnswer::parse(format!("answer{index}")).unwrap()),
            ))
        };

        assert!(free_input(20).is_ok());
        assert_eq!(
            free_input(21),
            Err(FreeInputError::TooManyExpectedAnswers { count: 21 })
        );
    }

    #[test]
    fn rejects_more_than_nine_wrong_choices() {
        let choice = |count: usize| {
            CharacterChoice::new(
                character('a'),
                BTreeSet1::from_head_and_tail(
                    character('b'),
                    (1..count).map(|index| character(char::from(b'b' + index as u8))),
                ),
            )
        };

        assert!(choice(9).is_ok());
        assert_eq!(
            choice(10),
            Err(CharacterChoiceError::TooManyWrongChoices { count: 10 })
        );
    }

    #[test]
    fn rejects_more_than_fifty_positions() {
        let position =
            || CharacterChoice::new(character('a'), BTreeSet1::from_one(character('b'))).unwrap();
        let choices = |count: usize| {
            CharacterChoices::new(Vec1::from_head_and_tail(
                position(),
                (1..count).map(|_| position()),
            ))
        };

        assert!(choices(50).is_ok());
        assert_eq!(
            choices(51),
            Err(CharacterChoicesError::TooManyCharacterChoices { count: 51 })
        );
    }

    #[test]
    fn rejects_answer_both_correct_and_wrong() {
        let tokyo = TextChoiceAnswer::parse("Tokyo").unwrap();

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
                BTreeSet1::from_one(TextChoiceAnswer::parse("Tokyo").unwrap()),
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
                BTreeSet1::from_one(TextChoiceAnswer::parse("correct").unwrap()),
                (0..wrong_count)
                    .map(|index| TextChoiceAnswer::parse(format!("wrong{index}")).unwrap())
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
            ImageChoiceAnswer::new(image, Some(ImageCaption::parse("Tokyo").unwrap())).unwrap();

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
    fn converts_a_complete_draft_response_mode() {
        let tokyo = FreeInputAnswer::parse("Tokyo").unwrap();
        let draft =
            DraftResponseMode::FreeInput(DraftFreeInput::new(BTreeSet::from([tokyo.clone()])));

        assert_eq!(
            ResponseMode::try_from(draft),
            Ok(ResponseMode::FreeInput(
                FreeInput::new(BTreeSet1::from_one(tokyo)).unwrap()
            ))
        );
    }

    #[test]
    fn rejects_a_draft_free_input_without_expected_answers() {
        let draft = DraftResponseMode::FreeInput(DraftFreeInput::new(BTreeSet::new()));

        assert_eq!(
            ResponseMode::try_from(draft),
            Err(ResponseModeFromDraftError::FreeInput(
                FreeInputFromDraftError::NoExpectedAnswer
            ))
        );
    }

    #[test]
    fn rejects_a_draft_choice_without_correct_answers() {
        let text_choice = DraftResponseMode::TextChoice(DraftTextChoice::new(
            ChoiceRequirement::One,
            BTreeSet::new(),
            BTreeSet::from([TextChoiceAnswer::parse("Kyoto").unwrap()]),
        ));
        let image_choice = DraftResponseMode::ImageChoice(DraftImageChoice::new(
            ChoiceRequirement::One,
            BTreeSet::new(),
            BTreeSet::new(),
        ));

        assert_eq!(
            ResponseMode::try_from(text_choice),
            Err(ResponseModeFromDraftError::TextChoice(
                TextChoiceFromDraftError::NoCorrectAnswer
            ))
        );
        assert_eq!(
            ResponseMode::try_from(image_choice),
            Err(ResponseModeFromDraftError::ImageChoice(
                ImageChoiceFromDraftError::NoCorrectAnswer
            ))
        );
    }

    #[test]
    fn rejects_a_draft_text_choice_with_an_answer_both_correct_and_wrong() {
        let tokyo = TextChoiceAnswer::parse("Tokyo").unwrap();
        let draft = DraftTextChoice::new(
            ChoiceRequirement::All,
            BTreeSet::from([tokyo.clone()]),
            BTreeSet::from([tokyo]),
        );

        assert_eq!(
            TextChoice::try_from(draft),
            Err(TextChoiceFromDraftError::AnswerBothCorrectAndWrong)
        );
    }

    #[test]
    fn rejects_draft_character_choices_without_positions() {
        let draft = DraftCharacterChoices::new(Vec::new());

        assert_eq!(
            CharacterChoices::try_from(draft),
            Err(CharacterChoicesFromDraftError::NoCharacterChoice)
        );
    }

    #[test]
    fn rejects_the_first_invalid_position_of_draft_character_choices() {
        let complete =
            DraftCharacterChoice::new(Some(character('a')), BTreeSet::from([character('b')]));
        let draft = DraftCharacterChoices::new(vec![
            complete,
            DraftCharacterChoice::new(Some(character('a')), BTreeSet::new()),
            DraftCharacterChoice::new(None, BTreeSet::new()),
        ]);

        assert_eq!(
            CharacterChoices::try_from(draft),
            Err(CharacterChoicesFromDraftError::InvalidCharacterChoice {
                position: 1,
                error: CharacterChoiceFromDraftError::NoWrongChoice,
            })
        );
    }

    #[test]
    fn reports_a_missing_correct_choice_before_missing_wrong_ones() {
        let draft = DraftCharacterChoice::new(None, BTreeSet::new());

        assert_eq!(
            CharacterChoice::try_from(draft),
            Err(CharacterChoiceFromDraftError::NoCorrectChoice)
        );
    }
}
