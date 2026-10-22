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
                "Click an element: a button, link, tab, radio or checkbox (choosing an answer or option), menu option, autocomplete suggestion or day."
            }
            Self::TypeText => {
                "Enter one of the supplied values into an editable field (replacing its text)."
            }
            Self::Select => "Pick an option in a native dropdown (a select box).",
            Self::PressEnter => "Press Enter in the field just typed into, to submit or accept it.",
            Self::ScrollDown => "Scroll the view down to reveal controls below.",
            Self::ScrollUp => "Scroll the view up to reveal controls above.",
            Self::Wait => "Wait: what the goal needs next is still loading or on its way.",
            Self::Done => "Every part of the goal is visibly satisfied in the view.",
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
    /// Whether the view changed after it.
    pub changed: Option<bool>,
    /// Why the view refused it (covered, disabled, gone).
    pub failed: Option<String>,
    /// It brought a whole new view (Next, Submit, following a link), not a change in the same one.
    pub new_view: bool,
}

impl GoalStep {
    /// `CLICK button "Search" [e7] · 91%`
    pub fn trace_line(&self) -> String {
        self.line(true)
    }

    /// The step as the next decision reads it: without its ref, which a new view reuses for
    /// another element.
    pub fn history_line(&self) -> String {
        self.line(false)
    }

    fn line(&self, with_ref: bool) -> String {
        let target = match (&self.element, &self.reference) {
            (Some(element), Some(reference)) if with_ref => format!(" {element} [{reference}]"),
            (Some(element), _) => format!(" {element}"),
            (None, Some(reference)) if with_ref => format!(" [{reference}]"),
            _ => String::new(),
        };
        let value = match (&self.value, self.operation) {
            (Some(seconds), Operation::Wait) => format!(" {seconds}s"),
            (Some(value), _) => format!(" ← \"{value}\""),
            (None, _) => String::new(),
        };
        let unchanged = match (&self.failed, self.changed) {
            (Some(error), _) => format!(" · failed: {error}"),
            _ if self.new_view => " · new view".into(),
            (None, Some(false)) => " · no visible change".into(),
            _ => String::new(),
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
    /// The view keeps coming back to the same state (an open-close loop): progress has stopped.
    Cycling,
    ActionFailed(String),
    NoDecision(String),
    /// The decision model could not be reached.
    Unreachable(String),
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
            Self::Cycling => f.write_str("the view keeps returning to the same state"),
            Self::ActionFailed(error) => write!(f, "the action failed: {error}"),
            Self::Unreachable(error) => write!(f, "the decision model is unreachable: {error}"),
            Self::NoDecision(error) => {
                write!(f, "the decision model gave no usable answer: {error}")
            }
            Self::Limit => f.write_str("the action budget ran out"),
        }
    }
}
