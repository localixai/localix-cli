//! What the person sends each lane, kept by the server (docs/daemon.md: the input queue is the
//! server's, the interface only shows it). An entry goes to the lane waiting for one; words sent
//! while a task works go to that task at its next step (steering); a command, a picture or Tab
//! waits for the task to end; what comes back from the background waits behind the person's own.
//! Every change, and only a change, is told to the interface as the lane's [`Event::Queue`]; a
//! lane's end as its [`Event::LaneEnded`].

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use protocol::{Event, Input};

/// One entry for a lane: what the person typed, or what came back from the background.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Typed {
    pub text: String,
    pub images: Vec<PathBuf>,
}

impl From<&str> for Typed {
    fn from(text: &str) -> Self {
        Self {
            text: text.into(),
            images: vec![],
        }
    }
}

/// Whether `entry` may go to the task running: words only.
pub(crate) fn steerable(entry: &Typed) -> bool {
    let text = entry.text.trim_start();
    entry.images.is_empty()
        && !text.is_empty()
        && !text.starts_with('/')
        && !text.starts_with('\u{1b}')
}

#[derive(Default)]
struct Lane {
    /// The lane waits for its next entry.
    ready: bool,
    /// A task of the lane runs: words the person sends are for it.
    busy: bool,
    queued: VecDeque<Typed>,
    steered: Vec<Typed>,
    background: VecDeque<Typed>,
    /// Its prompt gets nothing more: the person left, or the lane was closed.
    over: bool,
}

/// How a lane's queue is shown in the interface.
type Tell = Box<dyn Fn(&str, Event) + Send + Sync>;

/// Every lane's entries; `tell` shows a lane's queue (and what it read) in the interface.
#[derive(Default)]
struct Table {
    lanes: HashMap<String, Lane>,
    /// The person left: every prompt, a lane's first included, gets nothing until one is back.
    left: bool,
}

pub(crate) struct Inputs {
    lanes: Mutex<Table>,
    arrived: Condvar,
    tell: Tell,
}

impl Inputs {
    pub(crate) fn new(tell: impl Fn(&str, Event) + Send + Sync + 'static) -> Self {
        Self {
            lanes: Mutex::default(),
            arrived: Condvar::new(),
            tell: Box::new(tell),
        }
    }

    /// The next entry of `lane`, waiting for it; `None` once it gets nothing more. What the task
    /// before did not get to read is the first thing given.
    pub(crate) fn next(&self, lane: &str) -> Option<Typed> {
        let mut lanes = self.lock();
        let state = lanes.lanes.entry(lane.into()).or_default();
        for entry in std::mem::take(&mut state.steered).into_iter().rev() {
            state.queued.push_front(entry);
        }
        state.ready = true;
        let mut told = None;
        loop {
            let left = lanes.left;
            let state = lanes.lanes.entry(lane.into()).or_default();
            let taken = match state.over || left {
                true => Some(None),
                false => state
                    .queued
                    .pop_front()
                    .or_else(|| state.background.pop_front())
                    .map(Some),
            };
            if let Some(taken) = taken {
                state.ready = false;
                let queue = queue(state);
                drop(lanes);
                (self.tell)(lane, queue);
                return taken;
            }
            // Told once that it waits, outside the lock (R6); what arrives meanwhile wakes it.
            if told.is_none() {
                let queue = queue(state);
                drop(lanes);
                (self.tell)(lane, queue);
                told = Some(());
                lanes = self.lock();
                continue;
            }
            lanes = self
                .arrived
                .wait(lanes)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// What the person sent: for the lane waiting, the task running (words), or after it.
    pub(crate) fn input(&self, input: Input) {
        let entry = Typed {
            text: input.text,
            images: input.images,
        };
        self.change(&input.session, |state| {
            match !input.later && state.busy && steerable(&entry) {
                true => state.steered.push(entry),
                false => state.queued.push_back(entry),
            }
        });
    }

    /// What came back from the background (a monitor's lines, a schedule due): a task of its
    /// own, after what the person queued.
    pub(crate) fn background(&self, lane: &str, text: &str) {
        self.change(lane, |state| state.background.push_back(text.into()));
    }

    /// What the person said to the task running: taken (and shown as theirs), or, with `peek`,
    /// only looked at.
    pub(crate) fn steered(&self, lane: &str, peek: bool) -> Vec<String> {
        let mut texts = vec![];
        let mut take = |state: &mut Lane| {
            texts = state.steered.iter().map(|e| e.text.clone()).collect();
            if !peek {
                state.steered.clear();
            }
        };
        if peek {
            take(self.lock().lanes.entry(lane.into()).or_default());
            return texts;
        }
        self.change(lane, take);
        if !texts.is_empty() {
            (self.tell)(lane, Event::Steered(texts.clone()));
        }
        texts
    }

    /// A task of the lane began or ended: words sent meanwhile are for it.
    pub(crate) fn working(&self, lane: &str, busy: bool) {
        self.lock().lanes.entry(lane.into()).or_default().busy = busy;
    }

    /// The lane's prompt gets nothing more: it ends at its next wait. Only a lane that waited
    /// once is closed: the interface closes a lane at its prompt, and one gone is not made again.
    pub(crate) fn close(&self, lane: &str) {
        if let Some(state) = self.lock().lanes.get_mut(lane) {
            state.over = true;
        }
        self.arrived.notify_all();
    }

    /// The person left: every prompt gets nothing more, a lane's first one included.
    pub(crate) fn leave(&self) {
        self.lock().left = true;
        self.arrived.notify_all();
    }

    /// An interface is back (the sessions moved to the background): the prompts wait again.
    pub(crate) fn back(&self) {
        self.lock().left = false;
    }

    /// The lane ended: what it had waiting goes with it, and the interface lets it go.
    pub(crate) fn forget(&self, lane: &str) {
        self.lock().lanes.remove(lane);
        (self.tell)(lane, Event::LaneEnded);
    }

    /// `change` the lane's entries, then wake its prompt and, when its queue changed, tell the
    /// interface, outside the lock: a read that took nothing tells nothing.
    fn change(&self, lane: &str, change: impl FnOnce(&mut Lane)) {
        let mut lanes = self.lock();
        let state = lanes.lanes.entry(lane.into()).or_default();
        let before = queue(state);
        change(state);
        let queue = queue(state);
        drop(lanes);
        self.arrived.notify_all();
        if queue != before {
            (self.tell)(lane, queue);
        }
    }

    fn lock(&self) -> MutexGuard<'_, Table> {
        crate::sync::lock(&self.lanes)
    }
}

/// The lane's queue as the interface shows it: the background's by where it came from.
fn queue(state: &Lane) -> Event {
    let texts = |entries: &mut dyn Iterator<Item = &Typed>| -> Vec<String> {
        entries.map(|entry| entry.text.clone()).collect()
    };
    let background = state.background.iter().map(|e| super::wire::shown(&e.text));
    Event::Queue {
        ready: state.ready,
        steered: texts(&mut state.steered.iter()),
        queued: texts(&mut state.queued.iter()),
        background: background.collect(),
    }
}

#[cfg(test)]
#[path = "inputs_tests.rs"]
mod tests;
