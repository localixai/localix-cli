//! Stopping a task between model turns: when its time is up, when the person stopped it (Ctrl-C,
//! the browser window closed), or when no one reads its output any more (noticed on a write: with
//! `--json`, every event). The chat model is
//! wrapped, so the agent's loop ends the way it ends on any unreachable model, after the tool call
//! in flight has finished; nothing exits the process from another thread. (A stopped task's
//! running command is killed by the shell tool itself, and the gate approves nothing more.)

use std::cell::Cell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use contracts::chat::{Chat, ChatError, Completion, Request};
use contracts::model::{DecideError, Decider, Decision, Question};
use serde_json::Value;

use crate::interrupt::Interrupt;

/// Why a task was stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Halt {
    /// Its time limit ran out.
    TimedOut,
    /// The reader of its output went away.
    Closed,
    /// The person stopped it.
    Interrupted,
}

/// The chat model, stopped once the task's time is up or its output is closed.
pub struct Brake<'a> {
    pub chat: &'a dyn Chat,
    /// Whether the output's reader has gone.
    pub closed: &'a dyn Fn() -> bool,
    /// Whether the person stopped the task.
    pub stopped: &'a dyn Fn() -> bool,
    deadline: Cell<Option<Instant>>,
    halt: Cell<Option<Halt>>,
}

impl<'a> Brake<'a> {
    pub fn new(
        chat: &'a dyn Chat,
        closed: &'a dyn Fn() -> bool,
        stopped: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            chat,
            closed,
            stopped,
            deadline: Cell::new(None),
            halt: Cell::new(None),
        }
    }

    /// Start a task with `limit` to reach its answer in.
    pub fn arm(&self, limit: Duration) {
        self.deadline.set(Instant::now().checked_add(limit));
        self.halt.set(None);
    }

    /// Why the last task was stopped, if it was: a stop that came after its last turn (the window
    /// closed during the final tool call) still counts.
    pub fn halted(&self) -> Option<Halt> {
        self.halt
            .get()
            .or_else(|| (self.stopped)().then_some(Halt::Interrupted))
    }
}

impl Chat for Brake<'_> {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        let halt = if (self.stopped)() {
            Some(Halt::Interrupted)
        } else if (self.closed)() {
            Some(Halt::Closed)
        } else if self.deadline.get().is_some_and(|at| Instant::now() >= at) {
            Some(Halt::TimedOut)
        } else {
            None
        };
        match halt {
            Some(halt) => {
                self.halt.set(Some(halt));
                Err(ChatError::Unavailable(match halt {
                    Halt::TimedOut => "the time limit ran out".into(),
                    Halt::Closed => "the output was closed".into(),
                    Halt::Interrupted => "the person stopped the task".into(),
                }))
            }
            None => self.chat.complete(request),
        }
    }
}

/// The decision model behind the browser's hands, stopped with the task: the hands end their goal
/// at the next decision instead of acting on for dozens more steps.
pub struct BrakedDecider<'a> {
    pub inner: &'a dyn Decider,
    /// Shared with the hands' threads, so the interrupt itself rather than a closure over it.
    pub interrupt: &'a Interrupt,
}

impl Decider for BrakedDecider<'_> {
    fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision, DecideError> {
        if self.interrupt.stopped() {
            return Err(DecideError::Unavailable(
                "the person stopped the task".into(),
            ));
        }
        self.inner.decide(state, questions)
    }
}

#[cfg(test)]
#[path = "brake_tests.rs"]
mod tests;
