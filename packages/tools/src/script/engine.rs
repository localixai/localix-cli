//! A script run on QuickJS, from the caller's side. The engine lives on a thread of its own
//! (`runtime.rs`) and holds nothing it borrows: what it does reaches this thread as events on one
//! channel (a call to carry out, then the script's end), and the answers go back on another. A
//! call to a tool that may run beside others is carried out on a thread of its own, so a script's
//! independent calls run together; any other call runs here, alone. Meanwhile this thread watches
//! the stop and the deadline; either raises the engine's interrupt, which no `catch` in the script
//! can hold. The end is an event of its own, never the channel's closing: the script's functions,
//! and the sender they hold, are the engine's garbage to free whenever it likes.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, Scope};
use std::time::{Duration, Instant};

use contracts::tool::ToolError;
use serde_json::Value as Json;

use super::runtime::{Answer, evaluate};

/// How often each side looks at the stop and the deadline while the script runs.
pub(super) const POLL: Duration = Duration::from_millis(20);
/// How many answers may wait for the engine to take them.
const ANSWERS: usize = 64;

/// Why the engine's interrupt was raised; none while the script may run.
pub(super) const RUNNING: u8 = 0;
const STOPPED: u8 = 1;
const TIMED_OUT: u8 = 2;
const FATAL: u8 = 3;

pub(super) struct Limits<'a> {
    pub(super) deadline: Instant,
    pub(super) stop: &'a (dyn Fn() -> bool + Sync),
}

/// How a script ended other than with what it said.
pub(super) enum Ended {
    /// It threw, or did not parse: its message.
    Threw(String),
    Stopped,
    TimedOut,
    /// A tool it called failed the run: the run ends.
    Fatal(ToolError),
}

/// What the engine tells the caller's thread.
pub(super) enum Event {
    Call(Call),
    Ended(Result<String, Ended>),
}

/// A call the script makes, answered under its id.
pub(super) struct Call {
    pub(super) id: u64,
    pub(super) name: String,
    pub(super) arguments: String,
}

/// The run's tools as a script reaches them: each call carried out, and whether a tool may run
/// beside other calls.
pub(super) struct Serve<'a> {
    pub(super) call: &'a (dyn Fn(&str, &Json) -> Result<String, ToolError> + Sync),
    pub(super) parallel: &'a (dyn Fn(&str) -> bool + Sync),
}

/// Run `source` with `tools` named, each call carried out by `serve`; what it printed and
/// returned, cut as a command's output is.
pub(super) fn run(
    source: &str,
    tools: &[String],
    limits: &Limits,
    serve: &Serve,
) -> Result<String, Ended> {
    let halt = Arc::new(AtomicU8::new(RUNNING));
    let (events, heard) = mpsc::sync_channel::<Event>(1);
    let (answered, answers) = mpsc::sync_channel::<Answer>(ANSWERS);
    let fatal = Mutex::new(None);
    let ended = thread::scope(|scope| {
        let engine = {
            let halt = Arc::clone(&halt);
            thread::Builder::new()
                .name("exec".into())
                .spawn_scoped(scope, move || {
                    let ended = evaluate(source, tools, &halt, events.clone(), &answers);
                    // The caller waits for this event alone, so it is never gone before it.
                    let _ = events.send(Event::Ended(ended));
                })
                .map_err(|error| Ended::Threw(format!("the script could not start: {error}")))?
        };
        let ended = loop {
            match heard.recv_timeout(POLL) {
                Ok(Event::Call(call)) => {
                    let carry = Carry {
                        serve,
                        halt: &halt,
                        fatal: &fatal,
                        answered: answered.clone(),
                    };
                    carry.start(scope, call);
                }
                Ok(Event::Ended(ended)) => break ended,
                Err(RecvTimeoutError::Timeout) => {}
                // Every sender gone with no end said: the engine's thread panicked.
                Err(RecvTimeoutError::Disconnected) => {
                    break Err(Ended::Threw("the script's engine failed".into()));
                }
            }
            if (limits.stop)() {
                let _ = halt.compare_exchange(RUNNING, STOPPED, Ordering::SeqCst, Ordering::SeqCst);
            } else if Instant::now() >= limits.deadline {
                let _ =
                    halt.compare_exchange(RUNNING, TIMED_OUT, Ordering::SeqCst, Ordering::SeqCst);
            }
        };
        engine
            .join()
            .map_err(|failure| Ended::Threw(format!("the script's engine failed: {failure:?}")))
            .and(Ok(ended))
    })?;
    let fatal = fatal
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match (halt.load(Ordering::SeqCst), fatal) {
        (_, Some(error)) => Err(Ended::Fatal(error)),
        (STOPPED, None) => Err(Ended::Stopped),
        (TIMED_OUT, None) => Err(Ended::TimedOut),
        _ => ended,
    }
}

/// One call carried out, and its answer sent to the engine.
struct Carry<'s, 'a> {
    serve: &'s Serve<'a>,
    halt: &'s AtomicU8,
    fatal: &'s Mutex<Option<ToolError>>,
    answered: SyncSender<Answer>,
}

impl<'s> Carry<'s, '_> {
    /// On a thread of its own when the tool may run beside others, else here, before the next
    /// event.
    fn start<'scope>(self, scope: &'scope Scope<'scope, '_>, call: Call)
    where
        's: 'scope,
    {
        if !(self.serve.parallel)(&call.name) {
            return self.answer(&call);
        }
        let (id, answered) = (call.id, self.answered.clone());
        let started = thread::Builder::new()
            .name("exec-call".into())
            .spawn_scoped(scope, move || self.answer(&call));
        if let Err(error) = started {
            let _ = answered.send((id, Err(format!("the call could not start: {error}"))));
        }
    }

    fn answer(&self, call: &Call) {
        let answer = serde_json::from_str::<Json>(&call.arguments)
            .map_err(|error| format!("unusable arguments: {error}"))
            .and_then(
                |arguments| match (self.serve.call)(&call.name, &arguments) {
                    Ok(text) => Ok(text),
                    Err(ToolError::Failed(message)) => Err(message),
                    Err(error @ ToolError::Fatal(_)) => {
                        let message = error.to_string();
                        *crate::lock(self.fatal) = Some(error);
                        self.halt.store(FATAL, Ordering::SeqCst);
                        Err(message)
                    }
                },
            );
        // The engine is gone only when its script ended: nothing waits for this.
        let _ = self.answered.send((call.id, answer));
    }
}
