//! The kinds of tab the `window` tool opens, each a strategy over the engine that does the work:
//! a browser tab over the browser (`BrowserTool`, its page and its tabs), a terminal tab over a
//! terminal session (`bash`'s sessions). The window tool knows neither, and neither knows it.

mod changes;
mod missing;
#[cfg(unix)]
mod terminal;
mod tool;

pub(crate) use missing::Missing;
#[cfg(unix)]
pub(crate) use terminal::Terminal;
pub(crate) use tool::{Parts, window};

use std::sync::Mutex;

use contracts::tool::{Tool, ToolError, ToolOutput};
use contracts::view::{Action, Observation, View, ViewResult};
use serde_json::{Map, Value, json};
use tools::browser::{BrowserTool, Page};
use tools::window::{Kind, Opened};

use changes::Seen;

/// Where a browser tab opens when no address is given.
const SEARCH_PAGE: &str = "https://duckduckgo.com/";

/// The browser's locks: one for what changes its tabs (opening, closing), and one per tab, so
/// work in one tab never waits on another's; and the pages on disk it opened, served.
#[derive(Default)]
pub(crate) struct BrowserLocks {
    tabs_change: Mutex<()>,
    tabs: [Mutex<()>; tools::browser::MAX_TABS + 1],
    /// Each tab's page as the model last read it whole, and as the latest call left it.
    seen: Mutex<std::collections::HashMap<u32, Seen>>,
    served: tools::served::Served,
}

impl BrowserLocks {
    fn tab(&self, id: u32) -> std::sync::MutexGuard<'_, ()> {
        let lock = self.tabs.get(id as usize).unwrap_or(&self.tabs[0]);
        crate::sync::lock(lock)
    }
}

/// Browser tabs: the page's tabs, `w1`, `w2`, each worked in on its own, at once.
pub(crate) struct Browser<'a> {
    pub page: &'a Page,
    pub gate: &'a dyn contracts::gate::Gate,
    pub locks: &'a BrowserLocks,
    /// Where a page's path resolves, and what may not be served.
    pub workspace: &'a tools::workspace::Workspace,
}

impl Browser<'_> {
    /// Where `url` opens: a page on disk at the address it is served at, else the web address;
    /// and how the person reads it, for the gate.
    fn address(&self, url: &str) -> Result<(String, String), ToolError> {
        match self.locks.served.address(self.workspace, url)? {
            Some(served) => Ok((
                served.clone(),
                format!("{} (served at {served})", url.trim()),
            )),
            None => tools::browser::address(url).map(|web| (web.clone(), web)),
        }
    }

    /// `fields` of `args`.
    fn picked(args: &Value, fields: &[&str]) -> Value {
        let mut call = Map::new();
        for field in fields {
            if let Some(value) = args.get(*field) {
                call.insert((*field).into(), value.clone());
            }
        }
        Value::Object(call)
    }

    /// `arguments` carried out in tab `id`, while no other call works in it. A tab of a browser
    /// not open is none: acting in it would open the window on a page nobody asked for, beside
    /// the `open` that may be making it at the same time.
    fn in_tab(&self, id: u32, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let _tab = self.locks.tab(id);
        if !self.page.is_open() {
            return Err(ToolError::Failed(format!(
                "there is no browser tab w{id}: no page is open; `open` one first"
            )));
        }
        let page = self.page.pinned(id);
        let tool = BrowserTool {
            page: &page,
            gate: self.gate,
        };
        tool.call(arguments)
    }

    /// What acting in tab `id` left: the page's changes since the model last read it whole, when
    /// they are few; else the whole page, read from now on.
    fn after(&self, id: u32, mut done: ToolOutput) -> ToolOutput {
        let Some(page) = done.state.take() else {
            return done;
        };
        let mut seen = crate::sync::lock(&self.locks.seen);
        match seen.get_mut(&id).and_then(|before| before.after(&page)) {
            Some(changed) => done.report = format!("{}\n\n{changed}", done.report),
            None => {
                seen.insert(id, Seen::read(&page));
                done.state = Some(page);
            }
        }
        done
    }

    /// Tab `id` read whole: what later changes are told against.
    fn read_whole(&self, id: u32, done: ToolOutput) -> ToolOutput {
        if let Some(page) = &done.state {
            crate::sync::lock(&self.locks.seen).insert(id, Seen::read(page));
        }
        done
    }

    /// `arguments` that change the tabs (close one), one such call at a time.
    fn on_tabs(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let _change = crate::sync::lock(&self.locks.tabs_change);
        let tool = BrowserTool {
            page: self.page,
            gate: self.gate,
        };
        tool.call(arguments)
    }
}

impl Kind for Browser<'_> {
    fn name(&self) -> &'static str {
        "browser"
    }

    fn letter(&self) -> char {
        'w'
    }

    fn guide(&self) -> &'static str {
        tools::browser::GUIDE
    }

    fn fields(&self) -> Vec<(&'static str, Value)> {
        vec![
            (
                "url",
                json!({"type": "string", "description": "browser: an address to open, or an \
                HTML file's path."}),
            ),
            (
                "keys",
                json!({"type": "array", "items": {"type": "string"}, "description":
                "Keys to press, in order: a page's [\"ArrowUp\", \"Escape\", \"Ctrl+A\"], a \
                 terminal's [\"Enter\", \"Up\", \"Ctrl-C\"]."}),
            ),
            (
                "key_delay_ms",
                json!({"type": "integer", "minimum": 0, "maximum": 2000}),
            ),
            (
                "script",
                json!({"type": "string", "description": "browser: JavaScript run in the \
                page: an expression, or statements whose last value is returned (await works)."}),
            ),
            (
                "read",
                json!({"type": "boolean", "description": "browser: also the page's whole text."}),
            ),
            ("offset", json!({"type": "integer", "minimum": 0})),
            (
                "size",
                json!({"type": "string", "enum": ["desktop", "tablet", "mobile"]}),
            ),
            (
                "selector",
                json!({"type": "string", "description": "screenshot: crop it to the \
                first element this CSS selector matches."}),
            ),
            ("wait_for", tools::window::wait_for_field()),
            ("yield_ms", tools::window::yield_field()),
        ]
    }

    fn open(&self, args: &Value) -> Result<Opened, ToolError> {
        let url = args
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or(SEARCH_PAGE);
        let (url, shown) = self.address(url)?;
        // Asked before any lock: the person may take their time, and nobody else's tabs wait on
        // them (ARCHITECTURE.md R6).
        let effect = format!("open {shown} in a new tab");
        let asked = contracts::gate::Effect {
            tool: "browser",
            subject: "",
            summary: &effect,
        };
        if let contracts::gate::Verdict::Deny(why) = self.gate.check(asked) {
            return Err(ToolError::Failed(why));
        }
        let id = {
            // Making the tab is one at a time, with the look at whether the window is open that
            // decides how (a tab of it, or the window itself): one step (R7), so two first tabs
            // never open two windows, nor does one load in the other's tab. The lock is held for
            // the window's loop to make the tab, not for its page: tabs opened together load
            // together, each after its lock is let go.
            let _change = crate::sync::lock(&self.locks.tabs_change);
            (self.page.start(&url)).map_err(|error| ToolError::Failed(error.to_string()))?
        };
        let _ = self.page.pinned(id).driver().map(|tab| tab.wait_new());
        let read = self.in_tab(id, &Self::picked(args, &["wait_for", "yield_ms"]))?;
        let report = match read.report.is_empty() {
            true => format!("Opened {url}."),
            false => format!("Opened {url}.\n{}", read.report),
        };
        Ok(Opened {
            id,
            output: self.read_whole(id, ToolOutput { report, ..read }),
        })
    }

    fn interact(&self, id: u32, args: &Value) -> Result<ToolOutput, ToolError> {
        let fields = [
            "url",
            "keys",
            "key_delay_ms",
            "script",
            "read",
            "offset",
            "size",
            "wait_for",
            "yield_ms",
        ];
        let mut call = Self::picked(args, &fields);
        // A page on disk goes to the browser as the address it is served at.
        if let Some(url) = args.get("url").and_then(Value::as_str) {
            call["url"] = json!(self.address(url)?.0);
        }
        let done = self.in_tab(id, &call)?;
        Ok(self.after(id, done))
    }

    fn snapshot(&self, id: u32) -> Result<ToolOutput, ToolError> {
        let done = self.in_tab(id, &json!({}))?;
        Ok(self.read_whole(id, done))
    }

    fn screenshot(&self, id: u32, args: &Value) -> Result<ToolOutput, ToolError> {
        let mut call = Self::picked(args, &["selector"]);
        call["screenshot"] = json!(true);
        self.in_tab(id, &call)
    }

    fn close(&self, id: u32) -> Result<String, ToolError> {
        let _tab = self.locks.tab(id);
        self.on_tabs(&json!({"close_tab": id}))
            .map(|done| done.report)
    }

    fn tabs(&self) -> Vec<(u32, String, String)> {
        let Some(driver) = self.page.driver().ok().filter(|_| self.page.is_open()) else {
            return vec![];
        };
        let tabs = driver.tabs().0.into_iter();
        tabs.map(|tab| (tab.id, tab.title, tab.url)).collect()
    }

    fn view(&self, id: u32) -> Result<Box<dyn View + '_>, ToolError> {
        self.page
            .driver()
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        Ok(Box::new(Pinned {
            page: self.page.pinned(id),
            _tab: self.locks.tab(id),
        }))
    }
}

/// A tab's page, worked in alone while the helper acts on it.
struct Pinned<'a> {
    page: Page,
    _tab: std::sync::MutexGuard<'a, ()>,
}

impl View for Pinned<'_> {
    fn conventions(&self) -> &str {
        self.page.conventions()
    }
    fn observe(&self) -> ViewResult<Observation> {
        self.page.observe()
    }
    fn act(&self, epoch: &str, action: &Action) -> ViewResult<()> {
        self.page.act(epoch, action)
    }
}

#[cfg(test)]
#[path = "kinds_tests.rs"]
mod tests;
