//! What the agent sees of a page and what it can do to one, independent of the engine behind it.

mod guard;
mod host;
mod platform;
mod script;
mod viewport;
mod webview;

use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use host::{Window, run};
pub use viewport::Viewport;
pub use webview::WebviewDriver;

/// One atomic read of the page: the controls a person can see and use, and the text around them.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Observation {
    /// Changes with the document; an action carries it so it never lands on a newer page.
    pub epoch: String,
    pub url: String,
    pub title: String,
    pub text: String,
    pub viewport: Size,
    pub scroll: Scroll,
    pub elements: Vec<Element>,
    /// Visible controls left out past the element cap.
    #[serde(default)]
    pub omitted: usize,
    /// The page says it is still working (loading document, busy region, progress, "Loading…").
    #[serde(default)]
    pub busy: bool,
    /// Filled form fields, including those a modal hides from `elements`.
    #[serde(default)]
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Scroll {
    pub y: i64,
    pub height: i64,
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

/// One operation on the page. Targets are refs from the latest observation, never selectors.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Click { reference: String },
    Type { reference: String, text: String },
    Select { reference: String, value: String },
    PressEnter { reference: Option<String> },
    ScrollDown,
    ScrollUp,
}

/// Why the engine could not do what was asked.
#[derive(Debug, Clone, PartialEq)]
pub enum BrowserError {
    /// The page or element changed since the observation; observe again.
    Stale(String),
    /// The page refused the action (covered, disabled, not editable).
    Refused(String),
    /// The page did not answer in time, or the window is gone.
    Unavailable(String),
}

impl std::fmt::Display for BrowserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stale(message) | Self::Refused(message) | Self::Unavailable(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for BrowserError {}

pub type BrowserResult<T> = Result<T, BrowserError>;

/// A browser the agent drives: one page, observed and acted on in turn.
pub trait Driver {
    fn navigate(&self, url: &str) -> BrowserResult<()>;
    /// Wait until no load is in flight, up to `timeout`.
    fn wait_loaded(&self, timeout: Duration) -> BrowserResult<()>;
    fn observe(&self) -> BrowserResult<Observation>;
    /// Perform `action` against the observation with `epoch`, then let the page settle.
    fn act(&self, epoch: &str, action: &Action) -> BrowserResult<()>;
    /// Reframe the page to `width`×`height` (layout only: the user agent stays), then let it settle.
    fn resize(&self, width: f64, height: f64) -> BrowserResult<()>;
    /// Press `keys` (chords such as "ArrowUp", "Ctrl+A") on the focused element, `delay_ms` apart,
    /// then let the page settle.
    fn press(&self, keys: &[String], delay_ms: u64) -> BrowserResult<()>;
    /// Run `source` (JavaScript) in the page and return its value as text, then let the page settle.
    fn evaluate(&self, source: &str) -> BrowserResult<String>;
    /// The document's readable text from `offset`, at most `limit` characters.
    fn read(&self, offset: usize, limit: usize) -> BrowserResult<PageText>;
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PageText {
    pub text: String,
    pub total: usize,
    pub offset: usize,
}
