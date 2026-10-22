//! The vocabulary of a goal run: the operations a decision can name, a step as the trace and the
//! next request read it, and why a run stopped.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Click,
    TypeText,
    Select,
    PressEnter,
    ScrollDown,
    ScrollUp,
    Wait,
    Done,
    Blocked,
}

pub const OPERATIONS: [Operation; 9] = [
    Operation::Click,
    Operation::TypeText,
    Operation::Select,
    Operation::PressEnter,
    Operation::ScrollDown,
    Operation::ScrollUp,
    Operation::Wait,
    Operation::Done,
    Operation::Blocked,
];

impl Operation {
    pub fn key(self) -> &'static str {
        match self {
            Self::Click => "CLICK",
            Self::TypeText => "TYPE_TEXT",
            Self::Select => "SELECT",
            Self::PressEnter => "PRESS_ENTER",
            Self::ScrollDown => "SCROLL_DOWN",
            Self::ScrollUp => "SCROLL_UP",
            Self::Wait => "WAIT",
            Self::Done => "DONE",
            Self::Blocked => "BLOCKED",
        }
    }

    pub(crate) fn meaning(self) -> &'static str {
        match self {
            Self::Click => {
                "Click an element: a button, link, tab, menu option, autocomplete suggestion or day."
            }
            Self::TypeText => {
                "Enter one of the supplied values into an editable field (replacing its text)."
            }
            Self::Select => "Choose an option in a dropdown.",
            Self::PressEnter => "Press Enter in the field just typed into, to submit or accept it.",
            Self::ScrollDown => "Scroll the page down to reveal controls below.",
            Self::ScrollUp => "Scroll the page up to reveal controls above.",
            Self::Wait => "Wait briefly: submitted results or suggestions are still loading.",
            Self::Done => "Every part of the goal is visibly satisfied on the page.",
            Self::Blocked => "No offered operation can make progress.",
        }
    }

    pub(crate) fn scroll(self) -> bool {
        matches!(self, Self::ScrollDown | Self::ScrollUp)
    }
}

/// One executed (or final) step, as the trace and the next request's history read it.
#[derive(Debug, Clone, PartialEq)]
pub struct GoalStep {
    pub operation: Operation,
    pub reference: Option<String>,
    pub element: Option<String>,
    pub value: Option<String>,
    pub probability: f64,
    /// Whether the page changed after it.
    pub changed: Option<bool>,
}

impl GoalStep {
    /// `CLICK button "Search" [e7] · 91%`
    pub fn trace_line(&self) -> String {
        let target = match (&self.element, &self.reference) {
            (Some(element), Some(reference)) => format!(" {element} [{reference}]"),
            (None, Some(reference)) => format!(" [{reference}]"),
            _ => String::new(),
        };
        let value = self
            .value
            .as_ref()
            .map(|value| format!(" ← \"{value}\""))
            .unwrap_or_default();
        let unchanged = if self.changed == Some(false) {
            " · no visible change"
        } else {
            ""
        };
        let percent = (self.probability * 100.0).round();
        format!(
            "{}{target}{value} · {percent}%{unchanged}",
            self.operation.key()
        )
    }
}

/// What to do now, read from an answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    pub operation: Operation,
    pub reference: Option<String>,
    pub value: Option<String>,
    pub probability: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stop {
    Done {
        probability: f64,
    },
    Blocked,
    Repeated,
    /// The page keeps coming back to the same state (an open-close loop): progress has stopped.
    Cycling,
    /// The page moved to another site.
    Navigated(String),
    ActionFailed(String),
    NoDecision(String),
    Limit,
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Done { probability } if *probability < crate::goal::DONE_CONFIDENCE => write!(
                f,
                "the model said done but was unsure ({:.0}%): something may be missing",
                probability * 100.0
            ),
            Self::Done { .. } => f.write_str("the model judged the goal done"),
            Self::Blocked => f.write_str("the model is blocked: nothing offered can progress"),
            Self::Repeated => f.write_str("the same action twice without a visible change"),
            Self::Cycling => f.write_str("the page keeps returning to the same state"),
            Self::Navigated(url) => write!(f, "the page moved to another site ({url})"),
            Self::ActionFailed(error) => write!(f, "the action failed: {error}"),
            Self::NoDecision(error) => {
                write!(f, "the decision model gave no usable answer: {error}")
            }
            Self::Limit => write!(f, "the {}-action limit", crate::goal::MAX_ACTIONS),
        }
    }
}
