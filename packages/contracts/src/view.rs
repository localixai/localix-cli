//! The view protocol: what a view shows and what can be done to it, for the decision model that
//! drives it. A view shows controls by refs and is acted on by a ref and an operation, never a
//! selector or coordinates. A view knows nothing of who drives it.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// Something a decision model can see and act on: a document, a screen.
pub trait View {
    /// How the view's world behaves, where it is not obvious (how its forms or menus work).
    fn conventions(&self) -> &str {
        ""
    }
    fn observe(&self) -> ViewResult<Observation>;
    /// Perform `action` against the observation with `epoch`, then let the view settle.
    fn act(&self, epoch: &str, action: &Action) -> ViewResult<()>;
}

/// One atomic read of what the view shows: the controls one can see and use, and the text around.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Observation {
    /// Changes when the view shows a new document or screen; an action carries it so it never
    /// lands on a newer one.
    pub epoch: String,
    /// Where the view is, in its own terms (an address, a path, a screen).
    pub location: String,
    pub title: String,
    pub text: String,
    /// The view as an outline: headings and text in reading order, every control with its ref,
    /// indented by structure. Empty from a view that has none.
    #[serde(default)]
    pub outline: String,
    pub elements: Vec<Element>,
    /// The view says it is still working (loading, fetching, a progress bar, "Loading…").
    #[serde(default)]
    pub busy: bool,
    /// How far the view is scrolled: a scroll that moved nothing is told apart from one that did.
    #[serde(default)]
    pub scroll: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Element {
    #[serde(rename = "ref")]
    pub reference: String,
    pub role: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub editable: bool,
    /// In a dialog that covers the rest: while one is open, only its controls are offered.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dialog: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<SelectOption>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

impl Element {
    /// `textbox "Where to?" = "London" [expanded]`: how an element reads in requests and traces.
    pub fn describe(&self) -> String {
        let mut line = self.role.clone();
        if !self.name.is_empty() {
            let _ = write!(line, " \"{}\"", self.name);
        }
        if let Some(value) = self.value.as_deref().filter(|value| !value.is_empty()) {
            let _ = write!(line, " = \"{value}\"");
        }
        if !self.states.is_empty() {
            let _ = write!(line, " [{}]", self.states.join(", "));
        }
        line
    }

    pub fn disabled(&self) -> bool {
        self.states.iter().any(|state| state == "disabled")
    }
}

/// One operation on the view. Targets are refs from the latest observation.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Click { reference: String },
    Type { reference: String, text: String },
    Select { reference: String, value: String },
    PressEnter { reference: Option<String> },
    ScrollDown,
    ScrollUp,
}

/// Why the view could not do what was asked.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewError {
    /// What the action named changed since the observation; observe again.
    Stale(String),
    /// The view refused the action (covered, disabled, not editable).
    Refused(String),
    /// The view did not answer in time, or is gone.
    Unavailable(String),
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stale(message) | Self::Refused(message) | Self::Unavailable(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for ViewError {}

pub type ViewResult<T> = Result<T, ViewError>;
