//! What a run shows as it goes and how it ends. For a person: a trace on stderr (`▸ tool args`,
//! the browser hands' steps indented under their call) and the answer alone on stdout, so a pipe
//! gets the answer and nothing else. With `--json`: one JSON event per line on stdout
//! ([`json`]). A closed stdout is noticed when something is
//! written to it, never with a panic: with `--json` that is every event, so the run stops at its
//! next turn; otherwise only the answer is written, at the end, and the process then ends quietly.

pub(crate) mod human;
mod json;
mod observer;
mod spent;

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use agent::agent::Spend as AgentSpend;
use decision_loop::meter::Spend;
use decision_loop::step::GoalStep;
use serde_json::Value;

use ratatui::text::Line;

use crate::server::Ui;
use crate::tui::{cells, style};
use protocol::Event;

/// The trace's width where stderr is not a terminal.
const TRACE_WIDTH: usize = 100;

/// The columns of the terminal stderr goes to.
fn stderr_width() -> Option<usize> {
    #[cfg(unix)]
    {
        // SAFETY: a winsize is four integers: all zero is one.
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        // SAFETY: TIOCGWINSZ only fills the struct, which outlives the call.
        let ok = unsafe { libc::ioctl(2, libc::TIOCGWINSZ, &mut size) } == 0;
        (ok && size.ws_col > 0).then_some(size.ws_col as usize)
    }
    #[cfg(not(unix))]
    None
}

/// The `--json` event schema: a public contract (see CHANGELOG.md). 4: JSON lines replace the one
/// object of schema 3.
pub(crate) const SCHEMA: u32 = 4;

/// What a run starts with, as `run.started` reports it.
pub(crate) struct Start<'a> {
    pub goal: &'a str,
    pub session: &'a str,
    pub model: &'a str,
    pub decision_model: &'a str,
    pub workspace: &'a str,
}

/// How a run ended, as the closing lines and `run.completed` report it.
pub(crate) struct Summary<'a> {
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

#[allow(
    clippy::struct_excessive_bools,
    reason = "independent facts of the run: its options and where stdout and stderr go"
)]
pub(crate) struct Output {
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
    /// What the task did so far, each call by its id while it runs: for its session.
    done: Mutex<Vec<(String, contracts::history::Step)>>,
    /// stdout is a terminal: the answer is rendered there.
    answer_terminal: bool,
    /// What the session spent in all (`spent.rs`).
    total: Mutex<contracts::history::Spent>,
    /// Where stderr is read: a CI runner may take a line of it as a command.
    runner: Runner,
}

/// What reads the trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Runner {
    Plain,
    /// GitHub Actions takes a line that begins `::` or `##[` as a workflow command, on either
    /// stream. Only the trace on stderr is guarded: a file or an answer quoted in it must not be
    /// one (openai/codex-action#187). Stdout is the answer and the `--json` data, byte-exact for
    /// pipes: a caller that lets it reach the runner's log writes it to a file instead.
    GitHub,
}

impl Runner {
    fn here() -> Self {
        match std::env::var("GITHUB_ACTIONS").as_deref() {
            Ok("true") => Self::GitHub,
            _ => Self::Plain,
        }
    }

    /// `text` as this runner may read it: on GitHub, a zero-width space breaks each command's
    /// opening, which leaves what a person reads the same.
    fn inert(self, text: &str) -> String {
        match self {
            Self::Plain => text.to_string(),
            Self::GitHub => text
                .split_inclusive('\n')
                .map(|line| {
                    let body = line.trim_start();
                    let (indent, _) = line.split_at(line.len() - body.len());
                    match body
                        .strip_prefix("::")
                        .map(|rest| ("::", rest))
                        .or_else(|| body.strip_prefix("##[").map(|rest| ("##[", rest)))
                    {
                        Some((open, rest)) => {
                            let (first, second) = open.split_at(1);
                            format!("{indent}{first}\u{200b}{second}{rest}")
                        }
                        None => line.to_string(),
                    }
                })
                .collect(),
        }
    }
}

impl Output {
    /// Output on the process's stdout and stderr.
    pub(crate) fn new(json: bool, verbose: bool) -> Self {
        let out = Box::new(std::io::stdout());
        let mut output = Self::with(json, verbose, out, Box::new(std::io::stderr()));
        output.colour = style::colour_stderr();
        output.width = stderr_width().unwrap_or(TRACE_WIDTH);
        output.answer_terminal = std::io::IsTerminal::is_terminal(&std::io::stdout());
        output.runner = Runner::here();
        output
    }

    /// Output on given writers: `out` for the answer or the events, `err` for the trace.
    pub(crate) fn with(
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
            done: Mutex::default(),
            closed: AtomicBool::new(false),
            ui: None,
            colour: false,
            width: TRACE_WIDTH,
            blocks: AtomicUsize::new(0),
            steps: Mutex::new(vec![]),
            answer_terminal: false,
            total: Mutex::default(),
            runner: Runner::Plain,
        }
    }

    /// The run shown in the interface: blocks for the calls and the answer, nothing on stderr.
    pub(crate) fn in_ui(mut self, ui: Option<Ui>) -> Self {
        self.ui = ui;
        self
    }

    /// Whether the reader of stdout has gone.
    pub(crate) fn closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// A task starts: the trace's clock restarts.
    pub(crate) fn started(&self, start: &Start) {
        *lock(&self.started) = Instant::now();
        lock(&self.done).clear();
        if let Some(ui) = &self.ui {
            ui.send(Event::TaskStarted(start.goal.to_string()));
            return;
        }
        if self.json {
            self.event(json::started(start));
        }
    }

    /// One action of the browser hands.
    pub(crate) fn step(&self, step: &GoalStep) {
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
    pub(crate) fn ui(&self) -> Option<&Ui> {
        self.ui.as_ref()
    }

    /// One line under the call running (what an agent it started does): in the interface under
    /// the call, in the trace once the call is done; `--json` has the agent's turns instead.
    pub(crate) fn substep(&self, line: String) {
        if let Some(ui) = &self.ui {
            return ui.send(Event::Step(line));
        }
        if self.trace {
            lock(&self.steps).push(line);
        }
    }

    /// The task ended: the closing lines on stderr and the answer on stdout, or `run.completed`.
    pub(crate) fn completed(&self, summary: &Summary) {
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
    pub(crate) fn note(&self, text: &str) {
        if let Some(ui) = &self.ui {
            return ui.send(Event::Note(text.to_string()));
        }
        let text = self.runner.inert(text);
        let mut err = lock(&self.err);
        let _ = err.write_all(text.as_bytes()).and_then(|()| err.flush());
    }

    /// What the task did since it began, in the order each step began.
    pub(crate) fn take_steps(&self) -> Vec<contracts::history::Step> {
        let mut steps: Vec<_> = lock(&self.done).drain(..).map(|(_, step)| step).collect();
        steps.sort_by_key(|step| step.at_ms);
        steps
    }

    fn millis(&self) -> u64 {
        u64::try_from(lock(&self.started).elapsed().as_millis()).unwrap_or(u64::MAX)
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
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// `n` of `noun`, as the server says it (the interface counts its own): `1 task`, `3 tasks`.
pub(crate) fn count(n: usize, noun: &str) -> String {
    match n {
        1 => format!("1 {noun}"),
        n => format!("{n} {noun}s"),
    }
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
