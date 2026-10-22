//! How a command, a file change or a browser action is approved at the terminal, and nothing once
//! the task is stopped. Which effects are asked at all is the mode's (`mode::ModeGate`). The
//! question goes to the terminal itself (`/dev/tty`), so it works while stdin is a pipe, stdout
//! carries JSON and stderr goes to a file. What the question shows came from the model: its
//! control characters are escaped, so it reads as what will run.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::sync::Mutex;

use contracts::gate::{Effect, Gate, Verdict};

use crate::interrupt;
use crate::printable::printable;

/// Times an unreadable answer is asked again before it counts as no.
const TRIES: usize = 3;
/// How often a waiting question checks whether the task was stopped.
#[cfg(unix)]
const POLL_MS: libc::c_int = 100;
/// The same, as `select` takes it.
#[cfg(unix)]
const POLL_US: libc::suseconds_t = 100_000;

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
        // Stopped while the question was open: a late yes does not count, and a question that
        // Ctrl-C ended reads as the stop it was, not as a silent terminal.
        match self.inner.check(effect) {
            _ if (self.stopped)() => Verdict::Deny(STOPPED.into()),
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
#[derive(Debug, Clone, Copy)]
pub struct Tty;

impl Tty {
    /// Whether there is a terminal to ask on. Only the terminal itself counts, not where stdin or
    /// stderr go: a process with none (CI, cron, `nohup`) cannot open it.
    pub fn available() -> bool {
        Self::open().is_some()
    }

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
        if !typed(&input, &|| interrupt::PROCESS.stopped()) {
            return None;
        }
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

/// Wait until a line can be read from `input`; false when the task was stopped first. The signal
/// handler restarts a blocking read (`SA_RESTART`), so a read alone would keep the question open
/// after Ctrl-C until Enter: waiting in short polls lets the stop end it at once. macOS's `poll`
/// cannot wait on `/dev/tty` (it answers `POLLNVAL` at once), so there `select` waits instead.
#[cfg(unix)]
fn typed(input: &File, stopped: &dyn Fn() -> bool) -> bool {
    use std::os::fd::AsRawFd;
    let fd = input.as_raw_fd();
    let mut polls = true;
    loop {
        if stopped() {
            return false;
        }
        let ready = match polls {
            true => poll_in(fd),
            false => select_in(fd),
        };
        match ready {
            Readiness::Ready => return true,
            Readiness::Quiet => {}
            Readiness::Unsupported => polls = false,
        }
    }
}

/// What one short wait for input found.
#[cfg(unix)]
#[derive(Debug, PartialEq, Eq)]
enum Readiness {
    /// Input, a hangup or an error: the read that follows says which.
    Ready,
    /// Nothing within the wait, or a signal ended it.
    Quiet,
    /// This wait cannot be used on the descriptor.
    Unsupported,
}

#[cfg(unix)]
fn poll_in(fd: libc::c_int) -> Readiness {
    let mut wanted = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd, for a descriptor that stays open during the call.
    let ready = unsafe { libc::poll(&mut wanted, 1, POLL_MS) };
    if ready > 0 && wanted.revents & libc::POLLNVAL != 0 {
        return Readiness::Unsupported;
    }
    readiness(ready)
}

#[cfg(unix)]
fn select_in(fd: libc::c_int) -> Readiness {
    // A descriptor past what a set holds cannot be waited on: the read blocks, as on Windows.
    if usize::try_from(fd).map_or(true, |fd| fd >= libc::FD_SETSIZE) {
        return Readiness::Ready;
    }
    let mut timeout = libc::timeval {
        tv_sec: 0,
        tv_usec: POLL_US,
    };
    // SAFETY: the set is zeroed, then holds one descriptor below FD_SETSIZE; select reads and
    // writes only it and the timeout, both of which outlive the call.
    let ready = unsafe {
        let mut wanted: libc::fd_set = std::mem::zeroed();
        libc::FD_ZERO(&mut wanted);
        libc::FD_SET(fd, &mut wanted);
        let none = std::ptr::null_mut();
        libc::select(fd + 1, &mut wanted, none, none, &mut timeout)
    };
    readiness(ready)
}

#[cfg(unix)]
fn readiness(ready: libc::c_int) -> Readiness {
    let interrupted =
        ready < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted;
    match ready != 0 && !interrupted {
        true => Readiness::Ready,
        false => Readiness::Quiet,
    }
}

/// On Windows the read blocks: Ctrl-C ends the process there anyway (see `interrupt`).
#[cfg(not(unix))]
fn typed(_: &File, _: &dyn Fn() -> bool) -> bool {
    true
}

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

pub fn declined(why: Option<String>) -> String {
    match why {
        // What they said is the person speaking now, not a tool's error: it comes first.
        Some(why) => format!(
            "the person declined it and said: {why}\nThis is the person's instruction to you: \
             follow it before anything else."
        ),
        None => "the person declined it; do not try the same thing another way".into(),
    }
}

#[cfg(test)]
#[path = "gate_tests.rs"]
mod tests;
