//! Stopping a task between model turns: when its time is up, when the person stopped it (Ctrl-C),
//! or when no one reads its output any more (noticed on a write: with
//! `--json`, every event). The chat model is
//! wrapped, so the agent's loop ends the way it ends on any unreachable model, after the tool call
//! in flight has finished; nothing exits the process from another thread. (A stopped task's
//! running command is killed by the shell tool itself, and the gate approves nothing more.)

use std::sync::Mutex;
use std::time::{Duration, Instant};

use contracts::chat::{Chat, ChatError, Completion, Request};

use crate::sync::lock;

/// Why a task was stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Halt {
    /// Its time limit ran out.
    TimedOut,
    /// The reader of its output went away.
    Closed,
    /// The person stopped it.
    Interrupted,
}

/// The chat model, stopped once the task's time is up or its output is closed.
pub(crate) struct Brake<'a> {
    pub(crate) chat: &'a dyn Chat,
    /// Whether the output's reader has gone.
    pub(crate) closed: &'a (dyn Fn() -> bool + Sync),
    /// Whether the person stopped the task.
    pub(crate) stopped: &'a (dyn Fn() -> bool + Sync),
    deadline: Mutex<Option<Instant>>,
    halt: Mutex<Option<Halt>>,
}

impl<'a> Brake<'a> {
    pub(crate) fn new(
        chat: &'a dyn Chat,
        closed: &'a (dyn Fn() -> bool + Sync),
        stopped: &'a (dyn Fn() -> bool + Sync),
    ) -> Self {
        Self {
            chat,
            closed,
            stopped,
            deadline: Mutex::new(None),
            halt: Mutex::new(None),
        }
    }

    /// Start a task with `limit` to reach its answer in.
    pub(crate) fn arm(&self, limit: Duration) {
        *lock(&self.deadline) = Instant::now().checked_add(limit);
        *lock(&self.halt) = None;
    }

    /// Move the deadline `by` later: the time spent waiting on the person (an approval, a
    /// question) is not the task's, as the time at the prompt line is not.
    pub(crate) fn extend(&self, by: Duration) {
        let mut deadline = lock(&self.deadline);
        if let Some(at) = *deadline {
            *deadline = Some(at.checked_add(by).unwrap_or(at));
        }
    }

    /// Whether the task's time is up, recorded as its halt: a task that waits on its monitors or
    /// agents makes no model turn to be stopped at, so its wait asks this.
    pub(crate) fn due(&self) -> bool {
        let due = lock(&self.deadline).is_some_and(|at| Instant::now() >= at);
        if due {
            *lock(&self.halt) = Some(Halt::TimedOut);
        }
        due
    }

    /// Why the last task was stopped, if it was: a stop that came after its last turn (Ctrl-C
    /// during the final tool call) still counts.
    pub(crate) fn halted(&self) -> Option<Halt> {
        lock(&self.halt).or_else(|| (self.stopped)().then_some(Halt::Interrupted))
    }
}

impl Chat for Brake<'_> {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        let halt = if (self.stopped)() {
            Some(Halt::Interrupted)
        } else if (self.closed)() {
            Some(Halt::Closed)
        } else if lock(&self.deadline).is_some_and(|at| Instant::now() >= at) {
            Some(Halt::TimedOut)
        } else {
            None
        };
        match halt {
            Some(halt) => {
                *lock(&self.halt) = Some(halt);
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

#[cfg(test)]
#[path = "brake_tests.rs"]
mod tests;
