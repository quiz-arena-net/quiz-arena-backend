use std::{collections::BTreeSet, mem};

use isolang::Language;
use mitsein::{btree_set1::BTreeSet1, vec1::Vec1};
use uuid::Uuid;

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

/// Uniquely identifies the author of a quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct AuthorId(Uuid);

impl AuthorId {
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
pub(crate) struct Prompt(String);

impl Prompt {
    pub(crate) const MAX_LENGTH: usize = 1000;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, PromptError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(PromptError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(PromptError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(PromptError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PromptError {
    #[error("prompt must not be empty or only whitespace")]
    Blank,
    #[error(
        "prompt must be at most {} characters, got {length}",
        Prompt::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("prompt must not contain NUL")]
    InvalidCharacter,
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

/// Specifies how many correct choices a player must select in a multiple-choice
/// response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MultipleChoiceRequirement {
    /// Selecting any one correct answer is sufficient.
    One,

    /// Every correct answer must be selected.
    All,
}

/// An answer presented as a choice in a multiple-choice response.
///
/// Guaranteed to be 1 to 200 characters, not only whitespace, and free of NUL.
/// Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct MultipleChoiceAnswer(String);

impl MultipleChoiceAnswer {
    pub(crate) const MAX_LENGTH: usize = 200;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, MultipleChoiceAnswerError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(MultipleChoiceAnswerError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(MultipleChoiceAnswerError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(MultipleChoiceAnswerError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum MultipleChoiceAnswerError {
    #[error("multiple choice answer must not be empty or only whitespace")]
    Blank,
    #[error(
        "multiple choice answer must be at most {} characters, got {length}",
        MultipleChoiceAnswer::MAX_LENGTH
    )]
    TooLong { length: usize },
    #[error("multiple choice answer must not contain NUL")]
    InvalidCharacter,
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

/// The player selects from a collection of predefined answers.
///
/// No answer is both correct and wrong.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct MultipleChoice {
    /// Determines whether one or all correct answers must be selected.
    choice_requirement: MultipleChoiceRequirement,

    /// Choices that are considered correct.
    correct_answers: BTreeSet1<MultipleChoiceAnswer>,

    /// Choices that are considered incorrect.
    ///
    /// This set may be empty, for example when every presented choice is
    /// intentionally correct.
    wrong_answers: BTreeSet<MultipleChoiceAnswer>,
}

impl MultipleChoice {
    pub(crate) fn new(
        choice_requirement: MultipleChoiceRequirement,
        correct_answers: BTreeSet1<MultipleChoiceAnswer>,
        wrong_answers: BTreeSet<MultipleChoiceAnswer>,
    ) -> Result<Self, MultipleChoiceError> {
        if !correct_answers.is_disjoint(&wrong_answers) {
            return Err(MultipleChoiceError::AnswerBothCorrectAndWrong);
        }
        Ok(Self {
            choice_requirement,
            correct_answers,
            wrong_answers,
        })
    }

    pub(crate) fn choice_requirement(&self) -> MultipleChoiceRequirement {
        self.choice_requirement
    }

    pub(crate) fn correct_answers(&self) -> &BTreeSet1<MultipleChoiceAnswer> {
        &self.correct_answers
    }

    pub(crate) fn wrong_answers(&self) -> &BTreeSet<MultipleChoiceAnswer> {
        &self.wrong_answers
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum MultipleChoiceError {
    #[error("an answer must not be both correct and wrong")]
    AnswerBothCorrectAndWrong,
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
    MultipleChoice(MultipleChoice),
    CharacterChoices(CharacterChoices),
}

/// A tag used to classify or discover quizzes.
///
/// Guaranteed to be 1 to 32 characters, not only whitespace, and free of NUL.
/// Accepted text is preserved exactly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Tag(String);

impl Tag {
    pub(crate) const MAX_LENGTH: usize = 32;

    pub(crate) fn new(text: impl Into<String>) -> Result<Self, TagError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(TagError::Blank);
        }
        let length = text.chars().count();
        if length > Self::MAX_LENGTH {
            return Err(TagError::TooLong { length });
        }
        if text.contains('\0') {
            return Err(TagError::InvalidCharacter);
        }
        Ok(Self(text))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TagError {
    #[error("tag must not be empty or only whitespace")]
    Blank,
    #[error("tag must be at most {} characters, got {length}", Tag::MAX_LENGTH)]
    TooLong { length: usize },
    #[error("tag must not contain NUL")]
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
    author: AuthorId,

    /// The question text presented to players.
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
        author: AuthorId,
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
        author: AuthorId,
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

    pub(crate) fn author(&self) -> AuthorId {
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

    fn character(character: char) -> Character {
        Character::new(character).unwrap()
    }

    fn free_input() -> ResponseMode {
        ResponseMode::FreeInput(FreeInput::new(BTreeSet1::from_one(
            FreeInputAnswer::new("Tokyo").unwrap(),
        )))
    }

    fn multiple_choice() -> ResponseMode {
        ResponseMode::MultipleChoice(
            MultipleChoice::new(
                MultipleChoiceRequirement::One,
                BTreeSet1::from_one(MultipleChoiceAnswer::new("Tokyo").unwrap()),
                BTreeSet::from([MultipleChoiceAnswer::new("Kyoto").unwrap()]),
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
            AuthorId::from_uuid(Uuid::now_v7()),
            Prompt::new("What is the capital of Japan?").unwrap(),
            CanonicalAnswer::new("Tokyo").unwrap(),
            response_modes,
            Language::Eng,
            tags,
        )
    }

    #[test]
    fn accepts_text_within_bounds() {
        assert!(Prompt::new("x".repeat(1000)).is_ok());
        assert!(CanonicalAnswer::new("x".repeat(200)).is_ok());
        assert!(FreeInputAnswer::new("x".repeat(200)).is_ok());
        assert!(MultipleChoiceAnswer::new("x".repeat(200)).is_ok());
        assert!(Tag::new("x".repeat(32)).is_ok());
    }

    #[test]
    fn rejects_text_over_the_limit() {
        assert_eq!(
            Prompt::new("x".repeat(1001)),
            Err(PromptError::TooLong { length: 1001 })
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
            MultipleChoiceAnswer::new("x".repeat(201)),
            Err(MultipleChoiceAnswerError::TooLong { length: 201 })
        );
        assert_eq!(
            Tag::new("x".repeat(33)),
            Err(TagError::TooLong { length: 33 })
        );
    }

    #[test]
    fn counts_length_in_characters_not_bytes() {
        assert!(Tag::new("あ".repeat(32)).is_ok());
        assert_eq!(
            Tag::new("あ".repeat(33)),
            Err(TagError::TooLong { length: 33 })
        );
    }

    #[test]
    fn rejects_blank_text() {
        for text in ["", " ", "\t\n", "\u{3000}"] {
            assert_eq!(Prompt::new(text), Err(PromptError::Blank), "{text:?}");
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
                MultipleChoiceAnswer::new(text),
                Err(MultipleChoiceAnswerError::Blank),
                "{text:?}"
            );
            assert_eq!(Tag::new(text), Err(TagError::Blank), "{text:?}");
        }
    }

    #[test]
    fn rejects_text_containing_nul() {
        for text in ["\0", "a\0", "\0a", "a\0b"] {
            assert_eq!(
                Prompt::new(text),
                Err(PromptError::InvalidCharacter),
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
                MultipleChoiceAnswer::new(text),
                Err(MultipleChoiceAnswerError::InvalidCharacter),
                "{text:?}"
            );
            assert_eq!(Tag::new(text), Err(TagError::InvalidCharacter), "{text:?}");
        }
    }

    #[test]
    fn preserves_accepted_text_exactly() {
        for text in [" padded ", "Tokyo", "東京", "a\u{301}"] {
            assert_eq!(Prompt::new(text).unwrap().as_str(), text);
            assert_eq!(Tag::new(text).unwrap().as_str(), text);
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
        let tokyo = MultipleChoiceAnswer::new("Tokyo").unwrap();

        assert_eq!(
            MultipleChoice::new(
                MultipleChoiceRequirement::All,
                BTreeSet1::from_one(tokyo.clone()),
                BTreeSet::from([tokyo]),
            ),
            Err(MultipleChoiceError::AnswerBothCorrectAndWrong)
        );
    }

    #[test]
    fn accepts_multiple_choice_without_wrong_answers() {
        assert!(
            MultipleChoice::new(
                MultipleChoiceRequirement::All,
                BTreeSet1::from_one(MultipleChoiceAnswer::new("Tokyo").unwrap()),
                BTreeSet::new(),
            )
            .is_ok()
        );
    }

    #[test]
    fn creates_quiz_with_one_mode_of_each_kind() {
        let quiz = new_quiz(
            Vec1::from_head_and_tail(free_input(), [multiple_choice()]),
            tags(10),
        )
        .unwrap();

        assert_eq!(quiz.response_modes().len().get(), 2);
        assert_eq!(quiz.tags().len(), 10);
    }

    #[test]
    fn rejects_duplicate_response_mode_kind() {
        let response_modes =
            Vec1::from_head_and_tail(free_input(), [multiple_choice(), free_input()]);

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
                AuthorId::from_uuid(Uuid::now_v7()),
                Prompt::new("What is the capital of Japan?").unwrap(),
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

        quiz.change_prompt(Prompt::new("Capital of Japan?").unwrap());
        quiz.change_response_modes(Vec1::from_one(multiple_choice()))
            .unwrap();
        quiz.change_tags(tags(3)).unwrap();

        assert_eq!(quiz.prompt().as_str(), "Capital of Japan?");
        assert_eq!(quiz.response_modes().as_slice(), [multiple_choice()]);
        assert_eq!(quiz.tags().len(), 3);
    }
}
