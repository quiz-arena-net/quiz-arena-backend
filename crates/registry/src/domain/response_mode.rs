use std::collections::BTreeSet;

use mitsein::vec1::Vec1;

use super::media::{Media, MediaKind};

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
///
/// Guaranteed to hold at most 20 expected answers. The expected answers may be
/// empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FreeInput {
    /// Reference answers for judging.
    ///
    /// The actual judging strategy is configured separately by the session.
    expected_answers: BTreeSet<FreeInputAnswer>,
}

impl FreeInput {
    pub(crate) const MAX_EXPECTED_ANSWERS: usize = 20;

    pub(crate) fn new(expected_answers: BTreeSet<FreeInputAnswer>) -> Result<Self, FreeInputError> {
        let count = expected_answers.len();
        if count > Self::MAX_EXPECTED_ANSWERS {
            return Err(FreeInputError::TooManyExpectedAnswers { count });
        }
        Ok(Self { expected_answers })
    }

    pub(crate) fn expected_answers(&self) -> &BTreeSet<FreeInputAnswer> {
        &self.expected_answers
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
/// The correct answers may be empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct TextChoice {
    /// Determines whether one or all correct answers must be selected.
    choice_requirement: ChoiceRequirement,

    /// Choices that are considered correct.
    correct_answers: BTreeSet<TextChoiceAnswer>,

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
        correct_answers: BTreeSet<TextChoiceAnswer>,
        wrong_answers: BTreeSet<TextChoiceAnswer>,
    ) -> Result<Self, TextChoiceError> {
        if !correct_answers.is_disjoint(&wrong_answers) {
            return Err(TextChoiceError::AnswerBothCorrectAndWrong);
        }
        let count = correct_answers.len() + wrong_answers.len();
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

    pub(crate) fn correct_answers(&self) -> &BTreeSet<TextChoiceAnswer> {
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

/// The player selects from a collection of predefined image answers.
///
/// Guaranteed to hold at most 4 answers, no two of them with the same image,
/// which also keeps every answer from being both correct and wrong. The correct
/// answers may be empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ImageChoice {
    /// Determines whether one or all correct answers must be selected.
    choice_requirement: ChoiceRequirement,

    /// Choices that are considered correct.
    correct_answers: BTreeSet<ImageChoiceAnswer>,

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
        correct_answers: BTreeSet<ImageChoiceAnswer>,
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

    pub(crate) fn correct_answers(&self) -> &BTreeSet<ImageChoiceAnswer> {
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
/// One choice is correct. The remaining choices are distractors, and none of
/// them equals the correct one. Guaranteed to hold at most 9 wrong choices.
/// The correct choice may be missing and the wrong choices may be empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CharacterChoice {
    /// The correct character for this position.
    correct_choice: Option<Character>,

    /// Incorrect characters that may be presented alongside the correct one.
    wrong_choices: BTreeSet<Character>,
}

impl CharacterChoice {
    pub(crate) const MAX_WRONG_CHOICES: usize = 9;

    pub(crate) fn new(
        correct_choice: Option<Character>,
        wrong_choices: BTreeSet<Character>,
    ) -> Result<Self, CharacterChoiceError> {
        if correct_choice.is_some_and(|correct_choice| wrong_choices.contains(&correct_choice)) {
            return Err(CharacterChoiceError::CorrectChoiceAmongWrongChoices);
        }
        let count = wrong_choices.len();
        if count > Self::MAX_WRONG_CHOICES {
            return Err(CharacterChoiceError::TooManyWrongChoices { count });
        }
        Ok(Self {
            correct_choice,
            wrong_choices,
        })
    }

    pub(crate) fn correct_choice(&self) -> Option<Character> {
        self.correct_choice
    }

    pub(crate) fn wrong_choices(&self) -> &BTreeSet<Character> {
        &self.wrong_choices
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

/// The player constructs the answer one character at a time by selecting from a
/// set of candidate characters at each position.
///
/// Guaranteed to hold at most 50 positions. The positions may be empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CharacterChoices {
    /// The choices for each position.
    ///
    /// The order is significant as it corresponds to the order of characters in
    /// the answer.
    choices: Vec<CharacterChoice>,
}

impl CharacterChoices {
    pub(crate) const MAX_POSITIONS: usize = 50;

    pub(crate) fn new(choices: Vec<CharacterChoice>) -> Result<Self, CharacterChoicesError> {
        let count = choices.len();
        if count > Self::MAX_POSITIONS {
            return Err(CharacterChoicesError::TooManyPositions { count });
        }
        Ok(Self { choices })
    }

    pub(crate) fn choices(&self) -> &[CharacterChoice] {
        &self.choices
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum CharacterChoicesError {
    #[error(
        "character choices must have at most {} positions, got {count}",
        CharacterChoices::MAX_POSITIONS
    )]
    TooManyPositions { count: usize },
}

/// The way in which players respond to a quiz.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ResponseMode {
    FreeInput(FreeInput),
    TextChoice(TextChoice),
    ImageChoice(ImageChoice),
    CharacterChoices(CharacterChoices),
}

impl ResponseMode {
    /// Checks whether this response mode is ready for publication.
    ///
    /// Returns all violations instead of stopping at the first one.
    pub(crate) fn validate(&self) -> Result<(), Vec1<ResponseModeValidateError>> {
        let mut errors = Vec::new();
        match self {
            Self::FreeInput(free_input) => {
                if free_input.expected_answers.is_empty() {
                    errors.push(ResponseModeValidateError::FreeInputNoExpectedAnswer);
                }
            }
            Self::TextChoice(text_choice) => {
                if text_choice.correct_answers.is_empty() {
                    errors.push(ResponseModeValidateError::TextChoiceNoCorrectAnswer);
                }
            }
            Self::ImageChoice(image_choice) => {
                if image_choice.correct_answers.is_empty() {
                    errors.push(ResponseModeValidateError::ImageChoiceNoCorrectAnswer);
                }
            }
            Self::CharacterChoices(character_choices) => {
                if character_choices.choices.is_empty() {
                    errors.push(ResponseModeValidateError::CharacterChoicesNoPosition);
                }
                for (position, choice) in character_choices.choices.iter().enumerate() {
                    if choice.correct_choice.is_none() {
                        errors.push(
                            ResponseModeValidateError::CharacterChoiceNoCorrectCharacter {
                                position,
                            },
                        );
                    }
                    if choice.wrong_choices.is_empty() {
                        errors.push(ResponseModeValidateError::CharacterChoiceNoWrongCharacter {
                            position,
                        });
                    }
                }
            }
        }
        match Vec1::try_from(errors) {
            Ok(errors) => Err(errors),
            Err(_) => Ok(()),
        }
    }
}

/// A reason a response mode cannot be published.
///
/// Positions of character choices are zero-based indexes into
/// [`CharacterChoices::choices`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ResponseModeValidateError {
    #[error("a free input must have at least one expected answer")]
    FreeInputNoExpectedAnswer,
    #[error("a text choice must have at least one correct answer")]
    TextChoiceNoCorrectAnswer,
    #[error("an image choice must have at least one correct answer")]
    ImageChoiceNoCorrectAnswer,
    #[error("character choices must have at least one position")]
    CharacterChoicesNoPosition,
    #[error("character choice at position {position} must have a correct character")]
    CharacterChoiceNoCorrectCharacter { position: usize },
    #[error("character choice at position {position} must have at least one wrong character")]
    CharacterChoiceNoWrongCharacter { position: usize },
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
        assert!(FreeInputAnswer::new("x".repeat(200)).is_ok());
        assert!(TextChoiceAnswer::new("x".repeat(200)).is_ok());
        assert!(ImageCaption::new("x".repeat(200)).is_ok());
    }

    #[test]
    fn rejects_text_over_the_limit() {
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
    fn rejects_blank_text() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
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
        let wrong_choices = BTreeSet::from([character('a'), character('b')]);

        assert_eq!(
            CharacterChoice::new(Some(character('a')), wrong_choices),
            Err(CharacterChoiceError::CorrectChoiceAmongWrongChoices)
        );
    }

    #[test]
    fn accepts_distinct_character_choices() {
        let choice =
            CharacterChoice::new(Some(character('a')), BTreeSet::from([character('b')])).unwrap();

        assert_eq!(choice.correct_choice(), Some(character('a')));
    }

    #[test]
    fn rejects_more_than_twenty_expected_answers() {
        let free_input = |count: usize| {
            FreeInput::new(
                (0..count)
                    .map(|index| FreeInputAnswer::new(format!("answer{index}")).unwrap())
                    .collect(),
            )
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
                Some(character('a')),
                (0..count)
                    .map(|index| character(char::from(b'b' + index as u8)))
                    .collect(),
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
        let position = || {
            CharacterChoice::new(Some(character('a')), BTreeSet::from([character('b')])).unwrap()
        };
        let choices =
            |count: usize| CharacterChoices::new((0..count).map(|_| position()).collect());

        assert!(choices(50).is_ok());
        assert_eq!(
            choices(51),
            Err(CharacterChoicesError::TooManyPositions { count: 51 })
        );
    }

    #[test]
    fn rejects_answer_both_correct_and_wrong() {
        let tokyo = TextChoiceAnswer::new("Tokyo").unwrap();

        assert_eq!(
            TextChoice::new(
                ChoiceRequirement::All,
                BTreeSet::from([tokyo.clone()]),
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
                BTreeSet::from([TextChoiceAnswer::new("Tokyo").unwrap()]),
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
                BTreeSet::from([TextChoiceAnswer::new("correct").unwrap()]),
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
                BTreeSet::from([image_answer()]),
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
                BTreeSet::from([plain.clone(), labelled]),
                BTreeSet::new(),
            ),
            Err(ImageChoiceError::DuplicateImage)
        );
        assert_eq!(
            ImageChoice::new(
                ChoiceRequirement::One,
                BTreeSet::from([plain.clone()]),
                BTreeSet::from([plain]),
            ),
            Err(ImageChoiceError::DuplicateImage)
        );
    }

    #[test]
    fn validates_a_complete_response_mode() {
        let free_input =
            FreeInput::new(BTreeSet::from([FreeInputAnswer::new("Tokyo").unwrap()])).unwrap();

        assert_eq!(ResponseMode::FreeInput(free_input).validate(), Ok(()));
    }

    #[test]
    fn reports_a_blank_free_input() {
        let free_input = FreeInput::new(BTreeSet::new()).unwrap();

        assert_eq!(
            ResponseMode::FreeInput(free_input).validate(),
            Err(Vec1::from_one(
                ResponseModeValidateError::FreeInputNoExpectedAnswer
            ))
        );
    }

    #[test]
    fn reports_a_choice_without_correct_answers() {
        let text_choice = TextChoice::new(
            ChoiceRequirement::One,
            BTreeSet::new(),
            BTreeSet::from([TextChoiceAnswer::new("Kyoto").unwrap()]),
        )
        .unwrap();
        let image_choice =
            ImageChoice::new(ChoiceRequirement::One, BTreeSet::new(), BTreeSet::new()).unwrap();

        assert_eq!(
            ResponseMode::TextChoice(text_choice).validate(),
            Err(Vec1::from_one(
                ResponseModeValidateError::TextChoiceNoCorrectAnswer
            ))
        );
        assert_eq!(
            ResponseMode::ImageChoice(image_choice).validate(),
            Err(Vec1::from_one(
                ResponseModeValidateError::ImageChoiceNoCorrectAnswer
            ))
        );
    }

    #[test]
    fn reports_every_blank_in_character_choices() {
        let empty = CharacterChoices::new(Vec::new()).unwrap();
        let blank_positions = CharacterChoices::new(vec![
            CharacterChoice::new(None, BTreeSet::from([character('b')])).unwrap(),
            CharacterChoice::new(Some(character('a')), BTreeSet::new()).unwrap(),
            CharacterChoice::new(None, BTreeSet::new()).unwrap(),
        ])
        .unwrap();

        assert_eq!(
            ResponseMode::CharacterChoices(empty).validate(),
            Err(Vec1::from_one(
                ResponseModeValidateError::CharacterChoicesNoPosition
            ))
        );
        assert_eq!(
            ResponseMode::CharacterChoices(blank_positions).validate(),
            Err(Vec1::from_head_and_tail(
                ResponseModeValidateError::CharacterChoiceNoCorrectCharacter { position: 0 },
                [
                    ResponseModeValidateError::CharacterChoiceNoWrongCharacter { position: 1 },
                    ResponseModeValidateError::CharacterChoiceNoCorrectCharacter { position: 2 },
                    ResponseModeValidateError::CharacterChoiceNoWrongCharacter { position: 2 },
                ]
            ))
        );
    }
}
