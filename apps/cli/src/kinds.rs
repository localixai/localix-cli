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

/// Browser tabs: the page's tabs, `w1`, `w2`.
pub struct Browser<'a> {
    pub tool: &'a BrowserTool<'a>,
    pub page: &'a Page,
    /// One browser action at a time, every lane's and every agent's.
    pub lock: &'a Mutex<()>,
}

impl Browser<'_> {
    /// `fields` of `args`, and the tab `id` acted on first.
    fn on(&self, id: Option<u32>, args: &Value, fields: &[&str]) -> Value {
        let mut call = Map::new();
        if let Some(id) = id {
            call.insert("tab".into(), json!(id));
        }
        for field in fields {
            if let Some(value) = args.get(*field) {
                call.insert((*field).into(), value.clone());
            }
        }
        Value::Object(call)
    }

    fn call(&self, arguments: Value) -> Result<ToolOutput, ToolError> {
        let _one = crate::output::lock(self.lock);
        self.tool.call(&arguments)
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
            call["tab"] = json!("new");
        }
        let output = self.call(call)?;
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
        self.call(self.on(Some(id), args, &fields))
    }

    fn snapshot(&self, id: u32) -> Result<ToolOutput, ToolError> {
        self.call(json!({"tab": id}))
    }

    fn screenshot(&self, id: u32, args: &Value) -> Result<ToolOutput, ToolError> {
        let mut call = self.on(Some(id), args, &["selector"]);
        call["screenshot"] = json!(true);
        self.call(call)
    }

    fn close(&self, id: u32) -> Result<String, ToolError> {
        self.call(json!({"close_tab": id})).map(|done| done.report)
    }

    fn tabs(&self) -> Vec<(u32, String, String)> {
        let Some(driver) = self.page.driver().ok().filter(|_| self.page.is_open()) else {
            return vec![];
        };
        let tabs = driver.tabs().0.into_iter();
        tabs.map(|tab| (tab.id, tab.title, tab.url)).collect()
    }

    fn view(&self, id: u32) -> Result<Box<dyn View + '_>, ToolError> {
        let failed = |error: tools::browser::BrowserError| ToolError::Failed(error.to_string());
        let driver = self
            .page
            .driver()
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        driver.switch_tab(id).map_err(failed)?;
        Ok(Box::new(Locked {
            view: self.page,
            _one: crate::output::lock(self.lock),
        }))
    }
}

/// A view acted on alone: the browser's lock is held while the helper works.
struct Locked<'a> {
    view: &'a dyn View,
    _one: std::sync::MutexGuard<'a, ()>,
}

impl View for Locked<'_> {
    fn conventions(&self) -> &str {
        self.view.conventions()
    }
    fn observe(&self) -> ViewResult<Observation> {
        self.view.observe()
    }
    fn act(&self, epoch: &str, action: &Action) -> ViewResult<()> {
        self.view.act(epoch, action)
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
