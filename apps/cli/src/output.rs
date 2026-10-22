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
}

impl Output {
    /// Output on the process's stdout and stderr.
    pub fn new(json: bool, verbose: bool) -> Self {
        let out = Box::new(std::io::stdout());
        Self::with(json, verbose, out, Box::new(std::io::stderr()))
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
        }
    }

    /// Whether the reader of stdout has gone.
    pub fn closed(&self) -> bool {
        self.closed.get()
    }

    /// A task starts: the trace's clock restarts.
    pub fn started(&self, start: &Start) {
        self.started.set(Instant::now());
        if self.json {
            self.event(json::started(start));
        }
    }

    /// One action of the browser hands.
    pub fn step(&self, step: &GoalStep) {
        if self.json {
            self.event(json::step(step, self.millis()));
        }
        if self.trace {
            self.trace_line(&format!("    {}", step.trace_line()));
        }
    }

    /// The task ended: the closing lines on stderr and the answer on stdout, or `run.completed`.
    pub fn completed(&self, summary: &Summary) {
        if self.trace {
            self.note(&human::summary(summary));
        }
        if self.json {
            self.event(json::completed(summary));
            return;
        }
        let (stdout, stderr) = human::answer(summary);
        if !stderr.is_empty() {
            self.note(&stderr);
        }
        if !stdout.is_empty() {
            self.print(&format!("{stdout}\n"));
        }
    }

    /// Lines for the person on stderr, as they are.
    pub fn note(&self, text: &str) {
        let mut err = self.err.borrow_mut();
        let _ = err.write_all(text.as_bytes()).and_then(|()| err.flush());
    }

    fn millis(&self) -> u64 {
        self.started.get().elapsed().as_millis() as u64
    }

    fn trace_line(&self, text: &str) {
        let seconds = self.millis() as f64 / 1000.0;
        self.note(&format!("{seconds:>6.1}s {text}\n"));
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
        if self.json {
            self.event(json::call(call, self.millis()));
        }
        if self.trace {
            self.trace_line(&human::call(&call.name, &call.arguments));
        }
    }

    fn result(&self, call: &ToolCall, outcome: &Result<ToolOutput, ToolError>) {
        if self.json {
            self.event(json::result(call, outcome, self.millis()));
        }
        if self.trace {
            for line in human::result(outcome, self.verbose) {
                self.trace_line(&format!("    {line}"));
            }
        }
    }

    fn turn(&self, usage: &Usage) {
        if self.json {
            self.event(json::turn(usage, self.millis()));
        }
        if self.trace && self.verbose {
            self.trace_line(&human::turn(usage));
        }
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
