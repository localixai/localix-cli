//! What a run shows as it goes and how it ends. For a person: a trace on stderr (`▸ tool args`,
//! the browser hands' steps indented under their call) and the answer alone on stdout, so a pipe
//! gets the answer and nothing else. With `--json`: one JSON event per line on stdout, as
//! `codex exec --json` prints them ([`json`]). A closed stdout is noticed when something is
//! written to it, never with a panic: with `--json` that is every event, so the run stops at its
//! next turn; otherwise only the answer is written, at the end, and the process then ends quietly.

mod human;
mod json;

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::time::Instant;

use agent::agent::Spend as AgentSpend;
use agent::observer::Observer;
use contracts::chat::{ToolCall, Usage};
use contracts::tool::{ToolError, ToolOutput};
use decision_loop::meter::Spend;
use decision_loop::step::GoalStep;
use serde_json::Value;

use ratatui::text::{Line, Span};

use crate::tui::Ui;
use crate::tui::event::Event;
use crate::tui::tool_cell::{self, State};
use crate::tui::{cells, style, wrap};

/// The trace's width where stderr is not a terminal.
const TRACE_WIDTH: usize = 100;

/// The columns of the terminal stderr goes to.
fn stderr_width() -> Option<usize> {
    #[cfg(unix)]
    {
        // SAFETY: TIOCGWINSZ only fills the struct, which outlives the call.
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        let ok = unsafe { libc::ioctl(2, libc::TIOCGWINSZ, &mut size) } == 0;
        (ok && size.ws_col > 0).then_some(size.ws_col as usize)
    }
    #[cfg(not(unix))]
    None
}

/// The `--json` event schema: a public contract (see CHANGELOG.md). 4: JSON lines replace the one
/// object of schema 3.
pub const SCHEMA: u32 = 4;

/// What a run starts with, as `run.started` reports it.
pub struct Start<'a> {
    pub goal: &'a str,
    pub session: &'a str,
    pub model: &'a str,
    pub decision_model: &'a str,
    pub workspace: &'a str,
}

/// How a run ended, as the closing lines and `run.completed` report it.
pub struct Summary<'a> {
    /// `achieved`, `not_achieved`, `unstated`, `exhausted`, `error`, `interrupted`.
    pub status: &'a str,
    pub code: i32,
    pub answer: &'a str,
    /// `--schema`: the data, or why none matched.
    pub data: Option<&'a Result<Value, String>>,
    pub error: Option<&'a str>,
    pub agent: &'a AgentSpend,
    pub decision: &'a Spend,
    pub millis: u64,
    pub session: &'a str,
}

pub struct Output {
    /// JSON lines on stdout.
    json: bool,
    /// The trace on stderr.
    trace: bool,
    verbose: bool,
    out: RefCell<Box<dyn Write>>,
    err: RefCell<Box<dyn Write>>,
    started: Cell<Instant>,
    closed: Cell<bool>,
    /// In a session at a terminal: the interface shows the run instead of the trace.
    ui: Option<Ui>,
    /// The trace's colour and width, and how many blocks it printed.
    colour: bool,
    width: usize,
    blocks: Cell<usize>,
    /// The hands' actions under the call running, printed with it.
    steps: RefCell<Vec<String>>,
    /// stdout is a terminal: the answer is rendered there.
    answer_terminal: bool,
}

impl Output {
    /// Output on the process's stdout and stderr.
    pub fn new(json: bool, verbose: bool) -> Self {
        let out = Box::new(std::io::stdout());
        let mut output = Self::with(json, verbose, out, Box::new(std::io::stderr()));
        output.colour = style::colour_stderr();
        output.width = stderr_width().unwrap_or(TRACE_WIDTH);
        output.answer_terminal = std::io::IsTerminal::is_terminal(&std::io::stdout());
        output
    }

    /// Output on given writers: `out` for the answer or the events, `err` for the trace.
    pub fn with(json: bool, verbose: bool, out: Box<dyn Write>, err: Box<dyn Write>) -> Self {
        Self {
            json,
            trace: !json || verbose,
            verbose,
            out: RefCell::new(out),
            err: RefCell::new(err),
            started: Cell::new(Instant::now()),
            closed: Cell::new(false),
            ui: None,
            colour: false,
            width: TRACE_WIDTH,
            blocks: Cell::new(0),
            steps: RefCell::new(vec![]),
            answer_terminal: false,
        }
    }

    /// The run shown in the interface: blocks for the calls and the answer, nothing on stderr.
    pub fn in_ui(mut self, ui: Option<Ui>) -> Self {
        self.ui = ui;
        self
    }

    /// Whether the reader of stdout has gone.
    pub fn closed(&self) -> bool {
        self.closed.get()
    }

    /// A task starts: the trace's clock restarts.
    pub fn started(&self, start: &Start) {
        self.started.set(Instant::now());
        if let Some(ui) = &self.ui {
            ui.send(Event::TaskStarted(start.goal.to_string()));
            return;
        }
        if self.json {
            self.event(json::started(start));
        }
    }

    /// One action of the browser hands.
    pub fn step(&self, step: &GoalStep) {
        if let Some(ui) = &self.ui {
            return ui.send(Event::Step(step.trace_line()));
        }
        if self.json {
            self.event(json::step(step, self.millis()));
        }
        if self.trace {
            self.steps.borrow_mut().push(step.trace_line());
        }
    }

    /// The task ended: the closing lines on stderr and the answer on stdout, or `run.completed`.
    pub fn completed(&self, summary: &Summary) {
        if let Some(ui) = &self.ui {
            if let Some(error) = summary.error {
                ui.send(Event::Error(error.to_string()));
            }
            ui.send(Event::Answer(summary.answer.to_string()));
            return ui.send(Event::TaskEnded {
                status: summary.status.to_string(),
                millis: summary.millis,
                turns: summary.agent.calls,
                cost: summary.agent.cost + summary.decision.cost,
            });
        }
        // At a terminal the answer comes first, rendered, then how the run ended; a pipe gets
        // it as the model wrote it.
        let rendered = self.answer_terminal && summary.data.is_none() && !self.json;
        if rendered && !summary.answer.trim().is_empty() {
            let colour = style::colour_stdout();
            let rows: String = cells::agent(summary.answer, self.width)
                .iter()
                .map(|line| style::ansi(line, colour) + "\n")
                .collect();
            self.print(&format!("\n{rows}"));
        }
        if self.trace {
            let end = cells::End {
                status: summary.status,
                millis: summary.millis,
                turns: summary.agent.calls,
                cost: summary.agent.cost + summary.decision.cost,
            };
            self.block(&cells::end(&end, self.width));
            self.lines(&cells::note(&human::spend(summary), self.width));
            if let Some(error) = summary.error {
                self.lines(&cells::error(error, self.width));
            }
        }
        if self.json {
            self.event(json::completed(summary));
            return;
        }
        let (stdout, stderr) = human::answer(summary);
        if !stderr.is_empty() {
            self.note(&stderr);
        }
        if !stdout.is_empty() && !rendered {
            self.print(&format!("{stdout}\n"));
        }
    }

    /// Lines for the person on stderr, as they are.
    pub fn note(&self, text: &str) {
        if let Some(ui) = &self.ui {
            return ui.send(Event::Note(text.to_string()));
        }
        let mut err = self.err.borrow_mut();
        let _ = err.write_all(text.as_bytes()).and_then(|()| err.flush());
    }

    fn millis(&self) -> u64 {
        self.started.get().elapsed().as_millis() as u64
    }

    /// Rows on stderr, after a blank row when blocks came before.
    fn block(&self, lines: &[Line]) {
        if self.blocks.replace(self.blocks.get() + 1) > 0 {
            self.note("\n");
        }
        self.lines(lines);
    }

    fn lines(&self, lines: &[Line]) {
        let text: String = lines
            .iter()
            .map(|line| style::ansi(line, self.colour) + "\n")
            .collect();
        self.note(&text);
    }

    fn event(&self, event: Value) {
        self.print(&format!("{event}\n"));
    }

    /// Write to stdout; once its reader is gone, nothing more is written.
    fn print(&self, text: &str) {
        if self.closed.get() {
            return;
        }
        let mut out = self.out.borrow_mut();
        if out
            .write_all(text.as_bytes())
            .and_then(|()| out.flush())
            .is_err()
        {
            self.closed.set(true);
        }
    }
}

impl Observer for Output {
    fn call(&self, call: &ToolCall) {
        if let Some(ui) = &self.ui {
            return ui.send(Event::Call {
                name: call.name.clone(),
                args: call.arguments.clone(),
            });
        }
        if self.json {
            self.event(json::call(call, self.millis()));
        }
        if self.trace {
            // Printed whole once the call is done: a terminal's rows cannot be redrawn here.
            self.steps.borrow_mut().clear();
        }
    }

    fn result(&self, call: &ToolCall, outcome: &Result<ToolOutput, ToolError>) {
        if let Some(ui) = &self.ui {
            return ui.send(Event::Result {
                name: call.name.clone(),
                args: call.arguments.clone(),
                outcome: outcome.clone(),
            });
        }
        if self.json {
            self.event(json::result(call, outcome, self.millis()));
        }
        if self.trace {
            let done = State::Done(outcome);
            self.block(&tool_cell::head(
                &call.name,
                &call.arguments,
                &done,
                self.width,
            ));
            for step in self.steps.borrow_mut().drain(..) {
                let step = wrap::cut(&format!("  · {step}"), self.width);
                self.lines(&[Line::from(Span::styled(step, style::dim()))]);
            }
            self.lines(&tool_cell::body(&call.name, outcome, self.width));
        }
    }

    fn turn(&self, usage: &Usage) {
        if self.json {
            self.event(json::turn(usage, self.millis()));
        }
        if self.trace && self.verbose {
            self.lines(&cells::note(&human::turn(usage), self.width));
        }
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
