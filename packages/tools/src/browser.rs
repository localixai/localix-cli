//! The browser: a native window on the system WebView, observed and acted on through a probe
//! script in the page. [`BrowserTool`] is the agent's tool (`contracts::tool::Tool`); [`Page`] is
//! the window as a view (`contracts::view::View`) for the hands; [`Driver`] is the browser's own,
//! fuller surface (navigate, read, keys, scripts).

mod args;
mod capture;
mod comments;
mod deck;
mod feedback;
mod frame;
mod guard;
mod history;
mod host;
mod page;
mod panel;
mod platform;
mod script;
mod strip;
mod tab_view;
mod tabs;
mod told;
mod tool;
mod toolbar;
mod viewport;
mod webview;

use std::time::Duration;

use contracts::view::{Action, Element, ViewResult};
use serde::{Deserialize, Serialize};

pub use args::{GUIDE, address};
pub use capture::{Clip, Shot};
pub use host::{Browser, Dressing, Heard, Tab, Window, host};
pub use page::Page;
pub use panel::Panel;
pub use tabs::MAX_TABS;
pub use tool::BrowserTool;
pub use viewport::Viewport;
pub use webview::WebviewDriver;

/// Why there is no window where `has_display` is false, as the person and the model read it.
pub const NO_WINDOW: &str = "no display";

/// What would give it one.
pub const NO_WINDOW_HINT: &str = " (run under xvfb-run for it)";

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

impl From<Observation> for contracts::view::Observation {
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
            scroll: page.scroll.y,
        }
    }
}

/// A browser driven one page at a time: observed and acted on in turn.
pub trait Driver {
    fn navigate(&self, url: &str) -> ViewResult<()>;
    /// Wait until the document is usable (loaded, or parsed while images and frames still load),
    /// up to `timeout`.
    fn wait_loaded(&self, timeout: Duration) -> ViewResult<()>;
    fn observe(&self) -> ViewResult<Observation>;
    /// Perform `action` against the observation with `epoch`, then let the page settle.
    fn act(&self, epoch: &str, action: &Action) -> ViewResult<()>;
    /// Reframe the page to `width`×`height` (layout only: the user agent stays), then let it settle.
    fn resize(&self, width: f64, height: f64) -> ViewResult<()>;
    /// Press `keys` (chords such as "ArrowUp", "Ctrl+A") on the focused element, `delay_ms` apart,
    /// then let the page settle.
    fn press(&self, keys: &[String], delay_ms: u64) -> ViewResult<()>;
    /// Run `source` (JavaScript) in the page and return its value as text, then let the page settle.
    fn evaluate(&self, source: &str) -> ViewResult<String>;
    /// The document's readable text from `offset`, at most `limit` characters.
    fn read(&self, offset: usize, limit: usize) -> ViewResult<PageText>;
    /// A picture of the viewport as drawn, or of the first element matching the CSS `selector`
    /// (scrolled into view first, with a little of its surroundings).
    fn screenshot(&self, selector: Option<&str>) -> ViewResult<Shot>;
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PageText {
    pub text: String,
    pub total: usize,
    pub offset: usize,
}

/// With `BROWSER_DEBUG` set, `line` on stderr: a driver's timings and navigations, for whoever
/// debugs one.
#[expect(
    clippy::print_stderr,
    reason = "a developer's trace, printed only with BROWSER_DEBUG set (AGENTS.md)"
)]
fn debug(line: impl FnOnce() -> String) {
    if std::env::var_os("BROWSER_DEBUG").is_some() {
        eprintln!("{}", line());
    }
}
