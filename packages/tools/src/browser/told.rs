//! What the browser tells the app, as plain values with nothing of the engine in them: a tab as
//! the window lists it, a tab as the agent reads it, and what the person did in the window. The
//! headless build (`headless.rs`, ARCHITECTURE.md A8) includes this file as it is, so both builds
//! say it the same way.

/// Tabs open at once: each is a WebView with its own web process.
pub const MAX_TABS: usize = 8;

/// A tab of the window: its key (`w1`, a browser tab; `t2`, a terminal's screen), what kind it
/// is, its title, what it holds (a page's address), whether it is shown, and whether the browser
/// acts on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub id: String,
    pub kind: &'static str,
    pub title: String,
    pub detail: String,
    pub shown: bool,
    pub acted_on: bool,
    /// The project it was opened for, as its opener named it.
    pub group: String,
}

/// What the person did in the window, in words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard<'a> {
    /// They closed a tab, or the window: news for whoever works in it.
    Did(&'a str),
    /// They sent their comments on a page: words for the agent, as a message of theirs.
    Sent(&'a str),
}

/// One tab as the agent reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub id: u32,
    pub url: String,
    pub title: String,
}
