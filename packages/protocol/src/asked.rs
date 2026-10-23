//! What the server asks the person through the interface, and the replies. Each question belongs
//! to its session: it carries an id, the first reply wins, and the server may withdraw it (W5).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Asked {
    /// Whether an effect may happen.
    Approve { tool: String, summary: String },
    /// One question of the `ask` tool: the `at`th of `of`.
    Question {
        question: Question,
        at: usize,
        of: usize,
    },
    /// A choice of llx's own: a title and its options (a label and a note each), starting on
    /// `marked`.
    Choose {
        title: String,
        options: Vec<(String, String)>,
        marked: Option<usize>,
    },
    /// A choice with a value adjusted inline for each row.
    ChooseAdjustable {
        title: String,
        options: Vec<(String, String)>,
        marked: Option<usize>,
        adjustments: Vec<Adjustment>,
    },
    /// A line to type, shown as dots when it is a secret.
    Line { prompt: String, hidden: bool },
}

/// The reply to what was asked, of the same kind; `None` when the person cancelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum Reply {
    Approval(Approval),
    Question(Option<Answer>),
    Choice(Option<usize>),
    Adjusted(Option<(usize, usize)>),
    Line(Option<String>),
}

/// The answer to an approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "said", rename_all = "snake_case")]
pub enum Approval {
    Yes,
    /// Yes, and the same tool is not asked about again.
    Always,
    /// No, and what the person said beside it.
    No(Option<String>),
}

/// One question, with the options offered (none for an open question).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub text: String,
    pub choices: Vec<Choice>,
    /// Whether several options may be picked together.
    pub multiple: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub label: String,
    pub description: String,
}

/// The person's answer to one question: options, words of their own, both, or neither.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub choices: Vec<usize>,
    pub note: Option<String>,
}

/// A value a row adjusts inline: its choices and the one set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adjustment {
    pub values: Vec<String>,
    pub at: usize,
}
