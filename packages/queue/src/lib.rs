//! One queue for everything that reaches a session's run from outside its own calls: an agent's
//! answer, a monitor's lines, what the person did in the window, and whatever comes later. The
//! queue knows no source. Each entry carries its own policy, the one table every source
//! answers: whether a running task takes it at its next step, and what becomes of it while no
//! task runs. The reader (the agent's inbox) knows only entries.
//!
//! Nothing leaks: the queue is bounded (the oldest go first, and the reader is told how many),
//! an entry's body is data inside its tag and cannot close it, and a source's entries can be
//! forgotten with the source.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// What becomes of an entry that comes while no task runs, or that a task ended without reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Idle {
    /// It starts a task of its own (a monitor's lines).
    Opens,
    /// It waits for the next task, and joins it at its first step (what the person did).
    Waits,
    /// It was for the task that is over (an agent's late answer).
    Drops,
}

/// What a source says of its entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    /// A running task takes it at its next step.
    pub joins: bool,
    pub idle: Idle,
}

/// One thing that came, as its source made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub policy: Policy,
    /// What it is, and the tag it is read in (`agent_result`, `monitor`, `window`); empty for a
    /// line of the queue's own.
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub body: String,
    /// Its source (an agent's session, a job, a tab), to take or forget its entries by.
    pub key: Option<String>,
    /// A line before the tag: how to read it.
    pub preface: Option<String>,
}

impl Entry {
    pub fn new(policy: Policy, tag: &str, body: &str) -> Self {
        Self {
            policy,
            tag: tag.into(),
            attrs: vec![],
            body: body.into(),
            key: None,
            preface: None,
        }
    }

    pub fn keyed(mut self, key: &str) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn attr(mut self, name: &str, value: &str) -> Self {
        self.attrs.push((name.into(), value.into()));
        self
    }

    pub fn preface(mut self, text: &str) -> Self {
        self.preface = Some(text.into());
        self
    }

    /// As the model reads it: the body inside its tag, as data that cannot end the tag early.
    pub fn render(&self) -> String {
        if self.tag.is_empty() {
            return self.body.clone();
        }
        let attrs = self
            .attrs
            .iter()
            .fold(String::new(), |mut attrs, (name, value)| {
                let value = value.replace('"', "&quot;");
                // Writing to a String cannot fail.
                let _ = write!(attrs, " {name}=\"{value}\"");
                attrs
            });
        let close = format!("</{}", self.tag);
        let body = self.body.replace(&close, &format!("<\\/{}", self.tag));
        let tagged = format!("<{}{attrs}>\n{body}\n</{}>", self.tag, self.tag);
        match &self.preface {
            Some(preface) => format!("{preface}\n{tagged}"),
            None => tagged,
        }
    }
}

/// Called with an entry that opens a task of its own.
type Opener = Box<dyn Fn(Entry) + Send + Sync>;

#[derive(Default)]
struct State {
    entries: VecDeque<Entry>,
    /// A task runs.
    busy: bool,
    /// Entries dropped since the last take: the queue was full.
    dropped: usize,
}

pub struct Queue {
    state: Mutex<State>,
    wake: Condvar,
    capacity: usize,
    opener: Option<Opener>,
}

impl Queue {
    /// A queue that keeps at most `capacity` entries.
    pub fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            capacity: capacity.max(1),
            opener: None,
        }
    }

    /// Where an entry that opens a task goes (the interface, as a task of its own). With none, it
    /// waits for the next task instead.
    pub fn on_open(mut self, open: impl Fn(Entry) + Send + Sync + 'static) -> Self {
        self.opener = Some(Box::new(open));
        self
    }

    pub fn post(&self, entry: Entry) {
        let mut state = self.lock();
        if !state.busy {
            match (entry.policy.idle, &self.opener) {
                (Idle::Drops, _) => return,
                (Idle::Opens, Some(open)) => {
                    drop(state);
                    return open(entry);
                }
                _ => {}
            }
        }
        if state.entries.len() >= self.capacity {
            state.entries.pop_front();
            state.dropped += 1;
        }
        state.entries.push_back(entry);
        drop(state);
        self.wake.notify_all();
    }

    /// A task starts: what joins goes to it.
    pub fn begin(&self) {
        self.lock().busy = true;
    }

    /// The task ended: what it did not read opens a task, waits for the next, or goes.
    pub fn end(&self) {
        let mut state = self.lock();
        state.busy = false;
        let left: Vec<Entry> = state.entries.drain(..).collect();
        let mut opening = vec![];
        for entry in left {
            match (entry.policy.idle, &self.opener) {
                (Idle::Drops, _) => {}
                (Idle::Opens, Some(_)) => opening.push(entry),
                _ => state.entries.push_back(entry),
            }
        }
        drop(state);
        if let Some(open) = &self.opener {
            opening.into_iter().for_each(open);
        }
    }

    /// What a running task takes at its step, oldest first, with a line for any dropped.
    pub fn take_joining(&self) -> Vec<Entry> {
        let mut state = self.lock();
        let (taken, kept): (Vec<Entry>, Vec<Entry>) = state
            .entries
            .drain(..)
            .partition(|entry| entry.policy.joins);
        state.entries.extend(kept);
        let mut out = vec![];
        let dropped = std::mem::take(&mut state.dropped);
        if dropped > 0 && !taken.is_empty() {
            let note = format!(
                "({dropped} earlier message{} dropped: too many came at once)",
                if dropped == 1 { " was" } else { "s were" }
            );
            out.push(Entry::new(
                Policy {
                    joins: true,
                    idle: Idle::Drops,
                },
                "",
                &note,
            ));
        }
        out.extend(taken);
        out
    }

    /// The entries tagged `tag` of the sources `keys`, oldest first.
    pub fn take_keyed(&self, tag: &str, keys: &[&str]) -> Vec<Entry> {
        let mut state = self.lock();
        let wanted = |entry: &Entry| {
            entry.tag == tag && entry.key.as_deref().is_some_and(|key| keys.contains(&key))
        };
        let (taken, kept): (Vec<Entry>, Vec<Entry>) = state.entries.drain(..).partition(wanted);
        state.entries.extend(kept);
        taken
    }

    /// The sources that have entries tagged `tag` waiting.
    pub fn keys(&self, tag: &str) -> Vec<String> {
        let state = self.lock();
        let tagged = state.entries.iter().filter(|entry| entry.tag == tag);
        tagged.filter_map(|entry| entry.key.clone()).collect()
    }

    /// Drop the entries `which` says.
    pub fn forget(&self, which: impl Fn(&Entry) -> bool) {
        self.lock().entries.retain(|entry| !which(entry));
    }

    /// Wait up to `timeout` for an entry to come.
    pub fn wait(&self, timeout: Duration) {
        let state = self.lock();
        if state.entries.iter().any(|entry| entry.policy.joins) {
            return;
        }
        let _ = self.wake.wait_timeout(state, timeout);
    }

    /// Wake whoever waits, for a change the queue does not hold (an agent's task ended).
    pub fn poke(&self) {
        self.wake.notify_all();
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
