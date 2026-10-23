//! The windows a session shows by themselves: the browser's, and one per terminal session
//! `bash` keeps. They show where somebody watches: in a session, not in `exec` (nobody watches)
//! or autonomous mode (nobody is asked), and not over SSH, where they would open on a screen
//! the person is not at. `--show-browser` shows them anyway. The browser and the shell know
//! nothing of each other: the terminal windows are the browser host's panels, joined here.

use crate::cli::RunOptions;
use crate::mode::Mode;
#[cfg(unix)]
use tools::browser::Browser;
#[cfg(unix)]
use tools::shell::{Painter, Sessions, Theme, Watch};

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

/// Terminal windows as the browser host's panels, in the person's colours when known, drawn by
/// xterm.js when it is kept (checked as each opens), else as a plain grid. A panel closes when
/// its painter is dropped.
#[cfg(unix)]
pub fn panels(browser: Browser, theme: Option<Theme>) -> Watch {
    std::sync::Arc::new(move |title: &str| {
        let title = format!("llx · {title}");
        let emulator = crate::xterm::installed();
        let page = tools::shell::page(&title, theme.as_ref(), emulator.as_ref());
        let panel = browser.panel(&title, page, theme.map(|theme| theme.bg));
        Some(Box::new(move |frame| panel.paint(frame)) as Painter)
    })
}

#[cfg(all(test, unix))]
#[path = "windows_tests.rs"]
mod tests;
