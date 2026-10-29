//! Stopping a task between model turns: when its time is up, when its budget is spent, when the person stopped it (Ctrl-C),
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
    /// It spent its budget.
    OverBudget,
    /// The person stopped it.
    Interrupted,
}

/// What bounds a task here: its time, and the most it may spend. Children's requests go through
/// the same brake, so their spend counts.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    pub time: Duration,
    /// Dollars; `None` is no bound.
    pub budget: Option<f64>,
}

impl Limits {
    /// Bounded in time alone.
    #[cfg(test)]
    pub(crate) fn time(time: Duration) -> Self {
        Self { time, budget: None }
    }
}

/// The chat model, stopped once the task's time is up or its output is closed.
pub(crate) struct Brake<'a> {
    chat: &'a dyn Chat,
    /// Whether the output's reader has gone.
    closed: &'a (dyn Fn() -> bool + Sync),
    /// Whether the person stopped the task.
    stopped: &'a (dyn Fn() -> bool + Sync),
    deadline: Mutex<Option<Instant>>,
    /// The budget, and what the task has spent so far.
    budget: Mutex<(Option<f64>, f64)>,
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
            budget: Mutex::new((None, 0.0)),
            halt: Mutex::new(None),
        }
    }

    /// Start a task within `limits`; what it spent before is forgotten.
    pub(crate) fn arm(&self, limits: Limits) {
        *lock(&self.deadline) = Instant::now().checked_add(limits.time);
        *lock(&self.budget) = (limits.budget, 0.0);
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

    /// Whether the task's time is up or its budget spent, recorded as its halt: a task that
    /// waits on its monitors or agents makes no model turn to be stopped at, so its wait asks
    /// this.
    pub(crate) fn due(&self) -> bool {
        let late = lock(&self.deadline).is_some_and(|at| Instant::now() >= at);
        let (budget, spent) = *lock(&self.budget);
        let broke = budget.is_some_and(|most| spent >= most);
        let halt = if late {
            Some(Halt::TimedOut)
        } else {
            broke.then_some(Halt::OverBudget)
        };
        if halt.is_some() {
            *lock(&self.halt) = halt;
        }
        halt.is_some()
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
        } else if self.due() {
            *lock(&self.halt)
        } else {
            None
        };
        match halt {
            Some(halt) => {
                *lock(&self.halt) = Some(halt);
                Err(ChatError::Unavailable(match halt {
                    Halt::TimedOut => "the time limit ran out".into(),
                    Halt::OverBudget => "the budget ran out".into(),
                    Halt::Closed => "the output was closed".into(),
                    Halt::Interrupted => "the person stopped the task".into(),
                }))
            }
            None => {
                let done = self.chat.complete(request)?;
                lock(&self.budget).1 += done.usage.cost;
                Ok(done)
            }
        }
    }
}

#[cfg(test)]
#[path = "brake_tests.rs"]
mod tests;
