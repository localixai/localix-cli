//! The tool protocol: what a tool shows and what can be done to it. A tool shows controls by refs
//! and is acted on by a ref and an operation, never a selector or coordinates. A tool knows nothing
//! of who drives it.

use serde::{Deserialize, Serialize};

/// Something that can be driven toward a goal.
pub trait Tool {
    /// A short, unique name.
    fn name(&self) -> &str;
    /// What the tool is for, in a sentence: how a tool is chosen for a goal.
    fn description(&self) -> &str;
    /// How the tool's world behaves, where it is not obvious (how its forms or menus work).
    fn conventions(&self) -> &str {
        ""
    }
    /// Get ready for `goal`: open, or go where the goal names. `at` is where another tool left off
    /// (its `Observation::location`) when the goal moves here from it; a tool goes there if it
    /// can. Called before every goal, and on every move.
    fn start(&self, goal: &str, at: Option<&str>) -> ToolResult<()>;
    fn observe(&self) -> ToolResult<Observation>;
    /// Perform `action` against the observation with `epoch`, then let the tool settle.
    fn act(&self, epoch: &str, action: &Action) -> ToolResult<()>;
    /// Everything readable, beyond what is in view: what expectations are judged on.
    fn content(&self) -> ToolResult<String> {
        self.observe().map(|observation| observation.text)
    }
    /// A picture of what the tool shows, for the person; `None` from a tool that cannot take one.
    fn capture(&self) -> ToolResult<Option<Capture>> {
        Ok(None)
    }
}

/// A picture of a tool's view.
#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    /// `image/png`.
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}

/// One atomic read of what the tool shows: the controls one can see and use, and the text around.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Observation {
    /// Changes when the tool shows a new document or screen; an action carries it so it never
    /// lands on a newer one.
    pub epoch: String,
    /// Where the tool is, in its own terms (an address, a path, a screen).
    pub location: String,
    pub title: String,
    pub text: String,
    /// The view as an outline: headings and text in reading order, every control with its ref,
    /// indented by structure. Empty from a tool that has none.
    #[serde(default)]
    pub outline: String,
    pub elements: Vec<Element>,
    /// Visible controls left out past the element cap.
    #[serde(default)]
    pub omitted: usize,
    /// The tool says it is still working (loading, fetching, a progress bar, "Loading…").
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
            line.push_str(&format!(" \"{}\"", self.name));
        }
        if let Some(value) = self.value.as_deref().filter(|value| !value.is_empty()) {
            line.push_str(&format!(" = \"{value}\""));
        }
        if !self.states.is_empty() {
            line.push_str(&format!(" [{}]", self.states.join(", ")));
        }
        line
    }

    pub fn disabled(&self) -> bool {
        self.states.iter().any(|state| state == "disabled")
    }
}

/// One operation on the tool. Targets are refs from the latest observation.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Click { reference: String },
    Type { reference: String, text: String },
    Select { reference: String, value: String },
    PressEnter { reference: Option<String> },
    ScrollDown,
    ScrollUp,
}

/// Why the tool could not do what was asked.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolError {
    /// What the action named changed since the observation; observe again.
    Stale(String),
    /// The tool refused the action (covered, disabled, not editable).
    Refused(String),
    /// The tool did not answer in time, or is gone.
    Unavailable(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stale(message) | Self::Refused(message) | Self::Unavailable(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for ToolError {}

pub type ToolResult<T> = Result<T, ToolError>;
