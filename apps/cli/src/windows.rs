//! The window a session shows by itself: one, with the browser's tabs and a tab per terminal
//! session `bash` keeps. It shows where somebody watches: in a session, not in `exec` (nobody watches)
//! or autonomous mode (nobody is asked), and not over SSH, where they would open on a screen
//! the person is not at. `--show-browser` shows them anyway. The browser and the shell know
//! nothing of each other: the terminals' tabs are the browser host's panels, joined here.

use crate::cli::RunOptions;
use crate::mode::Mode;
#[cfg(unix)]
use tools::browser::Browser;
#[cfg(unix)]
use tools::shell::{Painter, Sessions, Theme, Watch};

/// The window as the `window` tool reads it: the browser host's tabs, of whatever kind.
pub struct Tabs(pub tools::browser::Browser);

impl tools::window::Windowed for Tabs {
    fn tabs(&self) -> Option<Vec<tools::window::WindowTab>> {
        let listed = self.0.listing()?;
        Some(
            listed
                .into_iter()
                .map(|tab| tools::window::WindowTab {
                    id: tab.id,
                    kind: tab.kind.to_string(),
                    title: tab.title,
                    detail: tab.detail,
                    shown: tab.shown,
                    acted_on: tab.acted_on,
                })
                .collect(),
        )
    }

    fn act(&self, act: tools::window::Act, tab: &str) -> Result<(), String> {
        self.0.act(tab, act == tools::window::Act::Close)
    }

    fn resize(&self, width: u32, height: u32) -> Result<(), String> {
        self.0.resize_window(f64::from(width), f64::from(height))
    }
}

/// Whether windows show by themselves.
pub fn shown(options: &RunOptions, asks: bool, started: Mode, remote: bool) -> bool {
    options.show_browser || (asks && started != Mode::Autonomous && !remote)
}

/// Whether llx runs over SSH, as `set` tells of the environment's variables.
pub fn remote(set: impl Fn(&str) -> bool) -> bool {
    ["SSH_CONNECTION", "SSH_TTY"].into_iter().any(set)
}

/// The terminal sessions `bash` keeps: each in a window through `watch` once kept, where windows
/// show; else in none, whatever a call asks.
#[cfg(unix)]
pub fn terminals(watch: Option<Watch>, shown: bool) -> Sessions {
    match watch.filter(|_| shown) {
        Some(watch) => Sessions::new(watch, true),
        None => Sessions::default(),
    }
}

/// The lanes, and the lane a terminal's tab is for.
#[cfg(unix)]
pub type Owner = (
    std::sync::Arc<crate::arrivals::Lanes>,
    std::sync::Weak<queue::Queue>,
);

/// Terminal windows as the browser host's panels, in the person's colours when known, drawn by
/// xterm.js when it is kept (checked as each opens), else as a plain grid. A panel closes when
/// its painter is dropped.
#[cfg(unix)]
pub fn panels(browser: Browser, theme: Option<Theme>, owner: Owner) -> Watch {
    std::sync::Arc::new(move |id: u32, title: &str| {
        let title = format!("llx · {title}");
        let emulator = crate::xterm::installed();
        let page = tools::shell::page(&title, theme.as_ref(), emulator.as_ref());
        // Its tab is known by the session's own id: `t3` is session 3.
        let panel = browser.panel(id, &title, page, theme.map(|theme| theme.bg));
        // What the person does to its tab reaches the lane that opened it.
        if let Some(queue) = owner.1.upgrade() {
            owner.0.own(&format!("t{}", panel.id()), &queue);
        }
        Some(Box::new(move |frame| panel.paint(frame)) as Painter)
    })
}

#[cfg(all(test, unix))]
#[path = "windows_tests.rs"]
mod tests;
