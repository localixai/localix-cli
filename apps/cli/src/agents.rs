//! `agents`: the main session starts other agents and works with them. An agent is a session of
//! its own with a parent, run on a thread of its own: `spawn` returns at once, and its answer
//! reaches the main agent at its next step, or through `wait`. Its conversation opens with the
//! same context, it works with tools of its own (neither `agents` nor `ask`: one level, and nobody
//! to answer it) behind the same gate, and it stops with the task or with `x` on its row. Its
//! tasks are recorded in its own session, which names its parent. A task does not end while its
//! agents work, and what they spend is the task's.

mod child;
pub(crate) mod inbox;
pub(crate) mod kit;
pub(crate) mod reported;
pub(crate) mod spec;
mod tool;
mod wait;
pub(crate) mod watch;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use agent::conversation::Conversation;
use contracts::chat::Spend;
use contracts::history::Session;
use queue::Queue;

use self::inbox::Letters;
pub(crate) use self::tool::AgentsTool;
use crate::mode::Mode;
use crate::sync::lock;
use protocol::Event;

/// Agents open at once: each keeps its conversation.
const MAX_OPEN: usize = 6;
/// How often a wait looks whether the task was stopped or the person wrote.
const POLL: Duration = Duration::from_millis(200);

/// What the task does with the agents of its session.
pub(crate) trait Delegation {
    /// Work for session `parent` from now on: the agents of another one are let go.
    fn adopt(&self, parent: &str);
    /// The conversation went back (`/rewind`): its agents are let go.
    fn forget(&self);
    /// The task ends: stop the agents still working and wait for them to end.
    fn settle(&self);
    /// What the agents spent since this was last asked.
    fn take_spent(&self) -> Spend;
}

/// What the tool and the agents' threads share.
pub(crate) struct Registry {
    state: Mutex<State>,
    /// The lane's queue: an agent's answer is posted there.
    pub(crate) queue: Arc<Queue>,
    /// An agent ended a task.
    changed: Condvar,
    /// The main agent's waiting line last said in its task: it is not said again until it changes.
    said: Mutex<String>,
}

#[derive(Default)]
struct State {
    /// The session the agents belong to.
    parent: String,
    children: Vec<Child>,
    spent: Spend,
}

struct Child {
    session: Session,
    /// Away while it works.
    conversation: Option<Conversation>,
    seen: HashSet<PathBuf>,
    /// The mode it last heard of.
    told: Mode,
    /// How its last task ended.
    status: String,
    open: bool,
    working: bool,
    stop: Arc<AtomicBool>,
    /// Its stop, known to the interface's keys while it is listed.
    _known: crate::interrupt::Known,
    /// What the main agent sends it while it works.
    letters: Arc<Letters>,
}

impl Registry {
    fn lock(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    /// How many agents work now.
    pub(crate) fn working(&self) -> usize {
        self.lock().children.iter().filter(|c| c.working).count()
    }

    /// Stop every agent: the session ends.
    pub(crate) fn stop_all(&self) {
        for child in &self.lock().children {
            child.stop.store(true, Ordering::SeqCst);
        }
    }

    /// The agents of the lane whose queue is `queue`.
    pub(crate) fn new(queue: Arc<Queue>) -> Self {
        Self {
            state: Mutex::default(),
            queue,
            changed: Condvar::new(),
            said: Mutex::default(),
        }
    }

    /// The task ended, however it ended: the next one says its waiting line afresh (an answer
    /// that came after its last step goes with the task, by its kind).
    pub(super) fn settled(&self) {
        lock(&self.said).clear();
    }

    /// Wait up to `timeout` for an agent to end a task.
    pub(crate) fn wait_change(&self, timeout: Duration) {
        let state = self.lock();
        let _ = self.changed.wait_timeout(state, timeout);
    }
}

impl Delegation for AgentsTool<'_, '_> {
    fn adopt(&self, parent: &str) {
        let mut state = self.registry.lock();
        if state.parent == parent {
            return;
        }
        state.parent = parent.to_string();
        drop(state);
        self.forget();
    }

    fn forget(&self) {
        let had = {
            let mut state = self.registry.lock();
            let had = !state.children.is_empty();
            for child in &state.children {
                child.stop.store(true, Ordering::SeqCst);
            }
            state.children.clear();
            had
        };
        // The state is let go before the queue and the interface are touched (R6).
        (self.registry.queue).forget(|entry| entry.tag == crate::arrivals::AGENT_RESULT);
        if had && let Some(ui) = self.kit.output.ui() {
            ui.send(Event::AgentsCleared);
        }
    }

    fn settle(&self) {
        {
            let state = self.registry.lock();
            for child in state.children.iter().filter(|c| c.working) {
                child.stop.store(true, Ordering::SeqCst);
            }
        }
        while self.registry.working() > 0 {
            self.registry.wait_change(POLL);
        }
        self.registry.settled();
    }

    fn take_spent(&self) -> Spend {
        std::mem::take(&mut self.registry.lock().spent)
    }
}

/// `wanted`, or `agent-N`, made unique among `children` with a number after it.
fn unique(wanted: Option<String>, children: &[Child]) -> String {
    let taken = |name: &str| children.iter().any(|child| child.session.name == name);
    let base = wanted.unwrap_or_else(|| format!("agent-{}", children.len() + 1));
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|name| !taken(name))
        .unwrap_or(base)
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
