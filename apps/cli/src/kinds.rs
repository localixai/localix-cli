//! The kinds of tab the `window` tool opens, each a strategy over the engine that does the work:
//! a browser tab over the browser (`BrowserTool`, its page and its tabs), a terminal tab over a
//! terminal session (`bash`'s sessions). The window tool knows neither, and neither knows it.

use std::sync::Mutex;

use contracts::tool::{Tool, ToolError, ToolOutput};
use contracts::view::{Action, Observation, View, ViewResult};
use serde_json::{Map, Value, json};
use tools::browser::{BrowserTool, Page};
use tools::window::{Kind, Opened};

/// Where a browser tab opens when no address is given.
const SEARCH_PAGE: &str = "https://duckduckgo.com/";

/// The browser's locks: one for what changes its tabs (opening, closing), and one per tab, so
/// work in one tab never waits on another's.
#[derive(Default)]
pub struct BrowserLocks {
    tabs_change: Mutex<()>,
    tabs: [Mutex<()>; tools::browser::MAX_TABS + 1],
}

impl BrowserLocks {
    fn tab(&self, id: u32) -> std::sync::MutexGuard<'_, ()> {
        let lock = self.tabs.get(id as usize).unwrap_or(&self.tabs[0]);
        crate::output::lock(lock)
    }
}

/// Browser tabs: the page's tabs, `w1`, `w2`, each worked in on its own, at once.
pub struct Browser<'a> {
    pub page: &'a Page,
    pub gate: &'a dyn contracts::gate::Gate,
    pub locks: &'a BrowserLocks,
}

impl Browser<'_> {
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

    /// `arguments` carried out in tab `id`, while no other call works in it.
    fn in_tab(&self, id: u32, arguments: Value) -> Result<ToolOutput, ToolError> {
        let _tab = self.locks.tab(id);
        let page = self.page.pinned(id);
        let tool = BrowserTool {
            page: &page,
            gate: self.gate,
        };
        tool.call(&arguments)
    }

    /// `arguments` that change the tabs (open one, close one), one such call at a time.
    fn on_tabs(&self, arguments: Value) -> Result<ToolOutput, ToolError> {
        let _change = crate::output::lock(&self.locks.tabs_change);
        let tool = BrowserTool {
            page: self.page,
            gate: self.gate,
        };
        tool.call(&arguments)
    }

    fn active(&self) -> Option<u32> {
        self.page.driver().ok().map(|driver| driver.tabs().1)
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
                json!({"type": "string", "description": "browser: an address to open."}),
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
        ]
    }

    fn open(&self, args: &Value) -> Result<Opened, ToolError> {
        let url = args
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or(SEARCH_PAGE);
        let mut call = json!({"url": url});
        if self.page.is_open() {
            call["new_tab"] = json!(true);
        }
        let output = self.on_tabs(call)?;
        let id = self
            .active()
            .ok_or_else(|| ToolError::Failed(output.report.clone()))?;
        Ok(Opened { id, output })
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
        ];
        self.in_tab(id, Self::picked(args, &fields))
    }

    fn snapshot(&self, id: u32) -> Result<ToolOutput, ToolError> {
        self.in_tab(id, json!({}))
    }

    fn screenshot(&self, id: u32, args: &Value) -> Result<ToolOutput, ToolError> {
        let mut call = Self::picked(args, &["selector"]);
        call["screenshot"] = json!(true);
        self.in_tab(id, call)
    }

    fn close(&self, id: u32) -> Result<String, ToolError> {
        let _tab = self.locks.tab(id);
        self.on_tabs(json!({"close_tab": id}))
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

/// Terminal tabs: a program in a terminal session, `t1`, `t2` (its session's id).
#[cfg(unix)]
pub struct Terminal<'a> {
    pub bash: &'a tools::shell::BashTool<'a>,
    pub sessions: &'a tools::shell::Sessions,
    pub stop: &'a (dyn Fn() -> bool + Sync),
}

#[cfg(unix)]
const TERMINAL_GUIDE: &str = "A program that must ask, in a real terminal (120×40): a REPL, an \
installer's questions, a login, another agent's interface. `open` with `command` (`workdir` to \
start elsewhere) returns once its output settles, or `wait_for` text shows. In `interact`: \
`input` types text and `keys` presses keys after it (submit with [\"Enter\"], not a newline); \
`wait_for` returns as soon as that text shows (the program's answer, not what was typed), \
`yield_ms` caps the wait. The snapshot is the transcript of plain output, or the screen itself \
when the program draws over its output: `screen 120×40, cursor at row R col C:` and its rows, \
`screen unchanged` when nothing moved. It runs on between tasks, so a REPL keeps its state: close \
it when done; at most 4 at once. Starting one and typing into it are approved first.";

#[cfg(unix)]
impl Terminal<'_> {
    fn call(&self, arguments: Value) -> Result<(ToolOutput, Option<u32>), ToolError> {
        self.bash.terminal(&arguments)
    }
}

#[cfg(unix)]
impl Kind for Terminal<'_> {
    fn name(&self) -> &'static str {
        "terminal"
    }

    fn letter(&self) -> char {
        't'
    }

    fn guide(&self) -> &'static str {
        TERMINAL_GUIDE
    }

    fn fields(&self) -> Vec<(&'static str, Value)> {
        vec![
            (
                "command",
                json!({"type": "string", "description": "terminal: the program to start."}),
            ),
            (
                "workdir",
                json!({"type": "string", "description": "terminal: where to start it, \
                relative to the workspace root."}),
            ),
            (
                "input",
                json!({"type": "string", "description": "terminal: text to type; a line \
                break is Enter. At most 4096 bytes."}),
            ),
            (
                "wait_for",
                json!({"type": "string", "description": "terminal: return as soon as \
                its output shows this exact text."}),
            ),
            (
                "yield_ms",
                json!({"type": "integer", "minimum": 100, "maximum": 30000}),
            ),
        ]
    }

    fn open(&self, args: &Value) -> Result<Opened, ToolError> {
        let mut call = json!({"tty": true});
        for field in ["command", "workdir", "wait_for", "yield_ms"] {
            if let Some(value) = args.get(field) {
                call[field] = value.clone();
            }
        }
        match self.call(call)? {
            (output, Some(id)) => Ok(Opened { id, output }),
            // It ended within the wait: no tab, and what it printed.
            (output, None) => Err(ToolError::Failed(output.report)),
        }
    }

    fn interact(&self, id: u32, args: &Value) -> Result<ToolOutput, ToolError> {
        let mut call = json!({"session": id});
        for field in ["input", "keys", "wait_for", "yield_ms"] {
            if let Some(value) = args.get(field) {
                call[field] = value.clone();
            }
        }
        self.call(call).map(|(output, _)| output)
    }

    fn snapshot(&self, id: u32) -> Result<ToolOutput, ToolError> {
        let screen = self.sessions.screen(id).map_err(ToolError::Failed)?;
        Ok(ToolOutput {
            report: String::new(),
            state: Some(screen),
            image: None,
        })
    }

    fn screenshot(&self, id: u32, _: &Value) -> Result<ToolOutput, ToolError> {
        let mut read = self.snapshot(id)?;
        read.report = "A terminal is read as text: its screen is below.".into();
        Ok(read)
    }

    fn close(&self, id: u32) -> Result<String, ToolError> {
        let closed = self.call(json!({"session": id, "close": true}))?;
        Ok(closed.0.report)
    }

    fn tabs(&self) -> Vec<(u32, String, String)> {
        let listed = self.sessions.listed().into_iter();
        listed
            .map(|(id, command)| (id, format!("session {id} · {command}"), String::new()))
            .collect()
    }

    fn view(&self, id: u32) -> Result<Box<dyn View + '_>, ToolError> {
        self.sessions.command(id).map_err(ToolError::Failed)?;
        Ok(Box::new(tools::shell::TerminalView {
            sessions: self.sessions,
            id,
            stop: self.stop,
        }))
    }
}

#[cfg(test)]
#[path = "kinds_tests.rs"]
mod tests;
