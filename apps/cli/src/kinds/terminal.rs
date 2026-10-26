//! Terminal tabs: the `window` tool's kind over `bash`'s terminal sessions.

use contracts::tool::{ToolError, ToolOutput};
use contracts::view::View;
use serde_json::{Value, json};
use tools::window::{Kind, Opened};

/// Terminal tabs: a program in a terminal session, `t1`, `t2` (its session's id).
pub(crate) struct Terminal<'a> {
    pub bash: &'a tools::shell::BashTool<'a>,
    pub sessions: &'a tools::shell::Sessions,
    pub stop: &'a (dyn Fn() -> bool + Sync),
}

const TERMINAL_GUIDE: &str = "A program that must ask, in a real terminal (120×40): a REPL, an \
installer's questions, a login, another agent's interface. `open` with `command` (`workdir` to \
start elsewhere) returns once its output settles, or `wait_for` text shows. In `interact`: \
`input` types text and `keys` presses keys after it (submit with [\"Enter\"], not a newline); \
`wait_for` returns as soon as that text shows (the program's answer, not what was typed), \
`yield_ms` caps the wait. The snapshot is the transcript of plain output, or the screen itself \
when the program draws over its output: `screen 120×40, cursor at row R col C:` and its rows, \
`screen unchanged` when nothing moved. It runs on between tasks, so a REPL keeps its state: close \
it when done; at most 4 at once. Starting one and typing into it are approved first.";

impl Terminal<'_> {
    fn call(&self, arguments: &Value) -> Result<(ToolOutput, Option<u32>), ToolError> {
        self.bash.terminal(arguments)
    }
}

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
            ("wait_for", tools::window::wait_for_field()),
            ("yield_ms", tools::window::yield_field()),
        ]
    }

    fn open(&self, args: &Value) -> Result<Opened, ToolError> {
        let mut call = json!({"tty": true});
        for field in ["command", "workdir", "wait_for", "yield_ms"] {
            if let Some(value) = args.get(field) {
                call[field] = value.clone();
            }
        }
        match self.call(&call)? {
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
        self.call(&call).map(|(output, _)| output)
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
        let closed = self.call(&json!({"session": id, "close": true}))?;
        Ok(closed.0.report)
    }

    fn tabs(&self) -> Vec<(u32, String, String)> {
        let listed = self.sessions.listed().into_iter();
        listed
            .map(|(id, command)| (id, format!("session {id} · {command}"), String::new()))
            .collect()
    }

    fn view(&self, id: u32) -> Result<Box<dyn View + '_>, ToolError> {
        let view = tools::shell::TerminalView::new(self.sessions, id, self.stop);
        Ok(Box::new(view.map_err(ToolError::Failed)?))
    }
}
