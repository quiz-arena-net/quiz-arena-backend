use std::collections::BTreeSet;

use isolang::Language;
use mitsein::{btree_set1::BTreeSet1, vec1::Vec1};
use uuid::Uuid;

/// Uniquely identifies a quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct QuizId(Uuid);

/// Uniquely identifies the author of a quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct AuthorId(Uuid);

/// The question text presented to players.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Prompt(String);

/// The canonical answer shown to players when the answer is revealed.
///
/// This is intended for presentation and does not necessarily correspond
/// exactly to the representation accepted by a particular [`ResponseMode`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CanonicalAnswer(String);

/// An expected textual answer for a [`ResponseMode::FreeInput`] response.
///
/// Whether a submitted response is considered correct is determined by the
/// judging strategy configured for the session. For example, a session may use
/// exact matching, host judgment, or an LLM-based judge.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FreeInputAnswer(String);

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
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct MultipleChoiceAnswer(String);

/// A single character used in a character-by-character response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Character(char);

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

/// The player provides an unrestricted textual response.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct FreeInput {
    /// Reference answers for judging.
    ///
    /// The actual judging strategy is configured separately by the session.
    expected_answers: BTreeSet1<FreeInputAnswer>,
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Tag(String);

/// A quiz that can be presented to players.
///
/// A quiz defines its authored content and the response modes it supports.
/// Session-specific behavior, such as how free-input responses are judged, is
/// configured outside the aggregate.
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
