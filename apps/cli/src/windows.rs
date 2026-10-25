//! The window a session shows by itself: one, with the browser's tabs and a tab per terminal
//! session `bash` keeps. It shows where somebody watches: in a session, not in `exec` (nobody watches),
//! and not over SSH, where they would open on a screen
//! the person is not at. `--show-browser` shows them anyway. The browser and the shell know
//! nothing of each other: the terminals' tabs are the browser host's panels, joined here.

use crate::cli::RunOptions;
#[cfg(unix)]
use tools::browser::Browser;
#[cfg(unix)]
use tools::shell::{Painter, Sessions, Theme, Watch};

/// The window as a lane's `window` tool reads it: the browser host's tabs, of whatever kind,
/// and the group (the lane's project) of those it opens.
pub(crate) struct Tabs {
    pub browser: Browser,
    pub group: String,
    /// Who hears what the person does to the tabs opened here: the lane's queue, or a tool
    /// client's.
    owner: Option<(
        std::sync::Arc<crate::arrivals::Lanes>,
        std::sync::Weak<queue::Queue>,
    )>,
}

impl Tabs {
    pub(crate) fn of(browser: Browser, group: &str) -> Self {
        let group = group.to_string();
        let owner = None;
        Self {
            browser,
            group,
            owner,
        }
    }

    /// The tabs opened here are `queue`'s: what the person does to them reaches it alone.
    pub(crate) fn owned(
        mut self,
        lanes: &std::sync::Arc<crate::arrivals::Lanes>,
        queue: &std::sync::Arc<queue::Queue>,
    ) -> Self {
        self.owner = Some((
            std::sync::Arc::clone(lanes),
            std::sync::Arc::downgrade(queue),
        ));
        self
    }
}

/// The group a lane's tabs are of: its session's directory's name, else the root's.
pub(crate) fn group(place: &str, root: &std::path::Path) -> String {
    let path = match place {
        "" => root,
        place => std::path::Path::new(place),
    };
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    name.unwrap_or_else(|| path.display().to_string())
}

impl tools::window::Windowed for Tabs {
    fn tabs(&self) -> Option<Vec<tools::window::WindowTab>> {
        let listed = self.browser.listing()?;
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
                    group: tab.group,
                })
                .collect(),
        )
    }

    fn act(&self, act: tools::window::Act, tab: &str) -> Result<(), String> {
        self.browser.act(tab, act == tools::window::Act::Close)
    }

    fn resize(&self, width: u32, height: u32) -> Result<(), String> {
        self.browser
            .resize_window(f64::from(width), f64::from(height))
    }

    fn opened(&self, tab: &str) {
        self.browser.group(tab, &self.group);
        if let Some((lanes, queue)) = &self.owner
            && let Some(queue) = queue.upgrade()
        {
            lanes.own(tab, &queue);
        }
    }
}

/// Whether windows show by themselves.
pub(crate) fn shown(options: &RunOptions, asks: bool, remote: bool) -> bool {
    options.show_browser || (asks && !remote)
}

/// Whether llx runs over SSH, as `set` tells of the environment's variables.
pub(crate) fn remote(set: impl Fn(&str) -> bool) -> bool {
    ["SSH_CONNECTION", "SSH_TTY"].into_iter().any(set)
}

/// The terminal sessions `bash` keeps: each in a window through `watch` once kept, where windows
/// show; else in none, whatever a call asks.
#[cfg(unix)]
pub(crate) fn terminals(watch: Option<Watch>, shown: bool) -> Sessions {
    match watch.filter(|_| shown) {
        Some(watch) => Sessions::new(watch, true),
        None => Sessions::default(),
    }
}

/// The lanes, the lane a terminal's tab is for, and its group.
#[cfg(unix)]
pub(crate) type Owner = (
    std::sync::Arc<crate::arrivals::Lanes>,
    std::sync::Weak<queue::Queue>,
    String,
);

/// Terminal windows as the browser host's panels, in the person's colours when known, drawn by
/// xterm.js when it is kept (checked as each opens), else as a plain grid. A panel closes when
/// its painter is dropped.
#[cfg(unix)]
pub(crate) fn panels(browser: Browser, theme: Option<Theme>, owner: Owner) -> Watch {
    std::sync::Arc::new(move |id: u32, title: &str| {
        let title = format!("llx · {title}");
        let emulator = crate::xterm::installed();
        let page = tools::shell::page(&title, theme.as_ref(), emulator.as_ref());
        // Its tab is known by the session's own id: `t3` is session 3, of the lane's project.
        browser.group(&format!("t{id}"), &owner.2);
        let panel = browser.panel(id, &title, page, theme.map(|theme| theme.bg));
        // What the person does to its tab reaches the lane that opened it.
        if let Some(queue) = owner.1.upgrade() {
            owner.0.own(&format!("t{}", panel.id()), &queue);
        }
        Some(Box::new(move |frame| panel.paint(frame)) as Painter)
    })
}

// Two attributes: `cfg(test)` alone is what marks test code for the lints.
#[cfg(test)]
#[cfg(unix)]
#[path = "windows_tests.rs"]
mod tests;
