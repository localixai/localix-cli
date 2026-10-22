//! The browser tool: a native window on the system WebView, observed and acted on through a probe
//! script in the page. [`BrowserTool`] implements the tool protocol (`agent::tool::Tool`);
//! [`Driver`] is the browser's own, fuller surface (navigate, read, keys, scripts).

mod guard;
mod host;
mod platform;
mod script;
mod start;
mod tool;
mod viewport;
mod webview;

use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use agent::tool::{
    Action, Element, Field, SelectOption, ToolError as BrowserError, ToolResult as BrowserResult,
};
pub use host::{Browser, Window, host, run};
pub use start::url_in;
pub use tool::BrowserTool;
pub use viewport::Viewport;
pub use webview::WebviewDriver;

/// Whether a window can be shown here: on Linux, WebKitGTK needs an X or Wayland server.
pub fn has_display() -> bool {
    !cfg!(target_os = "linux")
        || std::env::var_os("DISPLAY").is_some()
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// One atomic read of the page, as the probe takes it: the controls a person can see and use, the
/// text around them, and where the page is scrolled.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Observation {
    /// Changes with the document; an action carries it so it never lands on a newer page.
    pub epoch: String,
    pub url: String,
    pub title: String,
    pub text: String,
    /// The page as an outline: headings and text, controls with their refs (`[ref=e3]`).
    #[serde(default)]
    pub outline: String,
    pub viewport: Size,
    pub scroll: Scroll,
    pub elements: Vec<Element>,
    /// Visible controls left out past the element cap.
    #[serde(default)]
    pub omitted: usize,
    /// The page says it is still working (parsing, fetching its own data, a progress bar).
    #[serde(default)]
    pub busy: bool,
    /// Filled form fields, including those a modal hides from `elements`.
    #[serde(default)]
    pub fields: Vec<Field>,
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

impl From<Observation> for agent::tool::Observation {
    fn from(page: Observation) -> Self {
        Self {
            epoch: page.epoch,
            location: page.url,
            title: page.title,
            text: page.text,
            outline: page.outline,
            elements: page.elements,
            omitted: page.omitted,
            busy: page.busy,
            fields: page.fields,
            scroll: page.scroll.y,
        }
    }
}

/// A browser driven one page at a time: observed and acted on in turn.
pub trait Driver {
    fn navigate(&self, url: &str) -> BrowserResult<()>;
    /// Wait until the document is usable (loaded, or parsed while images and frames still load),
    /// up to `timeout`.
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
