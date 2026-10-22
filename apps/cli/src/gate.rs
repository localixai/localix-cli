//! Who approves a command, a file change or a browser action: `-y` allows everything
//! (`contracts::gate::Open`); at a terminal the person is asked; with no one to ask, it is refused
//! with a reason the model passes on. The question goes to the terminal itself (`/dev/tty`), so it
//! works while stdin is a pipe, stdout carries JSON and stderr goes to a file. What the question
//! shows came from the model: its control characters are escaped, so it reads as what will run.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::sync::Mutex;

use contracts::gate::{Effect, Gate, Open, Verdict};

use crate::printable::printable;

/// Times an unreadable answer is asked again before it counts as no.
const TRIES: usize = 3;

/// How commands and edits are approved in this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approvals {
    /// `-y`: allowed without asking.
    Yes,
    /// The person at the terminal answers each one.
    Ask,
    /// No one to ask: refused.
    Refuse,
}

impl Approvals {
    /// `-y` wins; otherwise the person is asked when a terminal is there to ask on. Only the
    /// terminal itself counts, not where stdin or stderr go: a process with none (CI, cron,
    /// `nohup`) cannot open it.
    pub fn choose(yes: bool) -> Self {
        Self::from(yes, Tty::open().is_some())
    }

    pub fn from(yes: bool, terminal: bool) -> Self {
        match (yes, terminal) {
            (true, _) => Self::Yes,
            (false, true) => Self::Ask,
            (false, false) => Self::Refuse,
        }
    }

    /// The approvals as the agent's context states them.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Yes => "commands, file changes and browser actions run without asking (--yes)",
            Self::Ask => {
                "the person approves each command, file change and browser action at the \
                 terminal; a refusal says why"
            }
            Self::Refuse => {
                "commands, file changes and browser actions are refused: no one is at a terminal \
                 to approve them, and the person must rerun with --yes to allow them; reading \
                 files works"
            }
        }
    }

    /// The gate that answers for these approvals.
    pub fn gate(self) -> Box<dyn Gate> {
        match self {
            Self::Yes => Box::new(Open),
            Self::Ask => Box::new(TerminalGate::new(Tty)),
            Self::Refuse => Box::new(Denying),
        }
    }
}

/// Refuses every effect, telling the model what the person must do to allow it.
pub struct Denying;

pub const NO_TERMINAL: &str = "no one is at a terminal to approve it; the person must rerun \
lx with --yes (-y) to allow commands, file changes and browser actions";

impl Gate for Denying {
    fn check(&self, _: Effect) -> Verdict {
        Verdict::Deny(NO_TERMINAL.into())
    }
}

/// Refuses everything once the person stopped the task, and asks `inner` until then: nothing
/// more runs after Ctrl-C, not even what was approved while the question was open.
pub struct Stoppable<'a> {
    pub inner: &'a dyn Gate,
    pub stopped: &'a dyn Fn() -> bool,
}

pub const STOPPED: &str = "the person stopped the task";

impl Gate for Stoppable<'_> {
    fn check(&self, effect: Effect) -> Verdict {
        if (self.stopped)() {
            return Verdict::Deny(STOPPED.into());
        }
        match self.inner.check(effect) {
            Verdict::Allow if (self.stopped)() => Verdict::Deny(STOPPED.into()),
            verdict => verdict,
        }
    }
}

/// Where a question is asked and its answer read.
pub trait Prompter {
    /// Show `question` and read one line; `None` when nothing can be read.
    fn ask(&self, question: &str) -> Option<String>;
}

/// The controlling terminal, opened for each question so nothing holds it between them.
pub struct Tty;

impl Tty {
    #[cfg(unix)]
    fn open() -> Option<(File, File)> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .ok()?;
        Some((file.try_clone().ok()?, file))
    }

    #[cfg(windows)]
    fn open() -> Option<(File, File)> {
        let input = OpenOptions::new()
            .read(true)
            .write(true)
            .open("CONIN$")
            .ok()?;
        let output = OpenOptions::new().write(true).open("CONOUT$").ok()?;
        Some((input, output))
    }
}

impl Prompter for Tty {
    fn ask(&self, question: &str) -> Option<String> {
        let (input, mut output) = Self::open()?;
        discard_typed_ahead(&input);
        output.write_all(question.as_bytes()).ok()?;
        output.flush().ok()?;
        let mut line = String::new();
        match BufReader::new(input).read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line),
        }
    }
}

/// Drop what was typed before the question was shown (the next task, typed while this one ran):
/// it must not answer a question the person never saw.
#[cfg(unix)]
fn discard_typed_ahead(input: &File) {
    use std::os::fd::AsRawFd;
    // SAFETY: tcflush only discards the terminal's pending input; the descriptor is open.
    unsafe { libc::tcflush(input.as_raw_fd(), libc::TCIFLUSH) };
}

#[cfg(not(unix))]
fn discard_typed_ahead(_: &File) {}

/// Asks the person for each effect; `a` allows the tool for the rest of the process.
pub struct TerminalGate<P: Prompter> {
    prompter: P,
    always: Mutex<HashSet<String>>,
}

impl<P: Prompter> TerminalGate<P> {
    pub fn new(prompter: P) -> Self {
        Self {
            prompter,
            always: Mutex::new(HashSet::new()),
        }
    }
}

impl<P: Prompter> Gate for TerminalGate<P> {
    fn check(&self, effect: Effect) -> Verdict {
        if self.always.lock().unwrap().contains(effect.tool) {
            return Verdict::Allow;
        }
        let tool = printable(effect.tool);
        let question = format!(
            "\n  {tool}: {}\n  allow? [y]es  [n]o (n <why> tells the agent)  [a]lways for {tool} > ",
            printable(effect.summary)
        );
        for _ in 0..TRIES {
            let Some(line) = self.prompter.ask(&question) else {
                return Verdict::Deny("no answer came from the terminal".into());
            };
            match answer(&line) {
                Some(Answer::Yes) => return Verdict::Allow,
                Some(Answer::Always) => {
                    self.always.lock().unwrap().insert(effect.tool.to_string());
                    return Verdict::Allow;
                }
                Some(Answer::No(why)) => return Verdict::Deny(declined(why)),
                None => {}
            }
        }
        Verdict::Deny(declined(None))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Answer {
    Yes,
    Always,
    /// No, and what the person said beside it.
    No(Option<String>),
}

/// The person's line as an answer; `None` when it is none of them. Approval is only the bare
/// word (`y`, `yes`, `a`, `always`): a line that says more ("yes, and then...") was likely meant
/// for something else, and is asked again. A refusal may say why (`n use make`). A bare Enter is
/// no: nothing runs that was not approved.
fn answer(line: &str) -> Option<Answer> {
    let line = line.trim();
    match line.to_ascii_lowercase().as_str() {
        "y" | "yes" => return Some(Answer::Yes),
        "a" | "always" => return Some(Answer::Always),
        _ => {}
    }
    let (word, rest) = line
        .split_once(|c: char| c.is_whitespace() || c == ':' || c == ',')
        .unwrap_or((line, ""));
    let rest = rest.trim_start_matches([':', ',']).trim();
    match word.to_ascii_lowercase().as_str() {
        "" | "n" | "no" => Some(Answer::No((!rest.is_empty()).then(|| rest.to_string()))),
        _ => None,
    }
}

fn declined(why: Option<String>) -> String {
    match why {
        Some(why) => format!("the person declined it and said: {why}"),
        None => "the person declined it; do not try the same thing another way".into(),
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
