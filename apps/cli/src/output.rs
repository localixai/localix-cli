//! What a run shows as it goes and how it ends. For a person: a trace on stderr (`▸ tool args`,
//! the browser hands' steps indented under their call) and the answer alone on stdout, so a pipe
//! gets the answer and nothing else. With `--json`: one JSON event per line on stdout
//! ([`json`]). A closed stdout is noticed when something is
//! written to it, never with a panic: with `--json` that is every event, so the run stops at its
//! next turn; otherwise only the answer is written, at the end, and the process then ends quietly.

pub mod human;
mod json;

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
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
    out: Mutex<Box<dyn Write + Send>>,
    err: Mutex<Box<dyn Write + Send>>,
    started: Mutex<Instant>,
    closed: AtomicBool,
    /// In a session at a terminal: the interface shows the run instead of the trace.
    ui: Option<Ui>,
    /// The trace's colour and width, and how many blocks it printed.
    colour: bool,
    width: usize,
    blocks: AtomicUsize,
    /// The hands' actions under the call running, printed with it.
    steps: Mutex<Vec<String>>,
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
    pub fn with(
        json: bool,
        verbose: bool,
        out: Box<dyn Write + Send>,
        err: Box<dyn Write + Send>,
    ) -> Self {
        Self {
            json,
            trace: !json || verbose,
            verbose,
            out: Mutex::new(out),
            err: Mutex::new(err),
            started: Mutex::new(Instant::now()),
            closed: AtomicBool::new(false),
            ui: None,
            colour: false,
            width: TRACE_WIDTH,
            blocks: AtomicUsize::new(0),
            steps: Mutex::new(vec![]),
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
        self.closed.load(Ordering::SeqCst)
    }

    /// A task starts: the trace's clock restarts.
    pub fn started(&self, start: &Start) {
        *lock(&self.started) = Instant::now();
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
            lock(&self.steps).push(step.trace_line());
        }
    }

    /// The interface the run is shown in, at a terminal.
    pub fn ui(&self) -> Option<&Ui> {
        self.ui.as_ref()
    }

    /// One line under the call running (what an agent it started does): in the interface under
    /// the call, in the trace once the call is done; `--json` has the agent's turns instead.
    pub fn substep(&self, line: String) {
        if let Some(ui) = &self.ui {
            return ui.send(Event::Step(line));
        }
        if self.trace {
            lock(&self.steps).push(line);
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
        let mut err = lock(&self.err);
        let _ = err.write_all(text.as_bytes()).and_then(|()| err.flush());
    }

    fn millis(&self) -> u64 {
        lock(&self.started).elapsed().as_millis() as u64
    }

    /// Rows on stderr, after a blank row when blocks came before.
    fn block(&self, lines: &[Line]) {
        if self.blocks.fetch_add(1, Ordering::SeqCst) > 0 {
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
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        let mut out = lock(&self.out);
        if out
            .write_all(text.as_bytes())
            .and_then(|()| out.flush())
            .is_err()
        {
            self.closed.store(true, Ordering::SeqCst);
        }
    }
}

/// `mutex` locked: a thread that panicked holding it left a value that is still whole.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
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
            lock(&self.steps).clear();
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
            let steps: Vec<String> = lock(&self.steps).drain(..).collect();
            for step in steps {
                let step = wrap::cut(&format!("  · {step}"), self.width);
                self.lines(&[Line::from(Span::styled(step, style::dim()))]);
            }
            self.lines(&tool_cell::body(&call.name, outcome, self.width));
        }
    }

    // `--json` has no event for it: the events are a contract, changed only with the schema.
    fn said(&self, text: &str) {
        if let Some(ui) = &self.ui {
            return ui.send(Event::Answer(text.to_string()));
        }
        if self.trace {
            self.block(&cells::agent(text, self.width));
        }
    }

    fn fitted(&self, report: &agent::history::Report) {
        let said: Vec<String> = report
            .iter()
            .map(|(stage, count)| format!("{count} {stage}"))
            .collect();
        self.note(&format!(
            "✻ The context neared the model's limit; elided to save it: {}\n",
            said.join(", ")
        ));
    }

    fn turn(&self, usage: &Usage) {
        // A lane's tokens: its agents' turns come here too.
        if let Some(ui) = &self.ui {
            ui.send(Event::Spent(usage.input_tokens + usage.output_tokens));
        }
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
