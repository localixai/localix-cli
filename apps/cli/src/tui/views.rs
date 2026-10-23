//! What the interface keeps of each session: a lane (a session the person started) or an agent
//! it started, each with its transcript, and which one is shown.

use std::collections::VecDeque;
use std::sync::mpsc::SyncSender;
use std::time::Instant;

use ratatui::text::Line;

use super::event::Typed;

/// Every session the interface knows, and which one it shows.
pub(crate) struct Views {
    /// Each lane, followed by the agents it started, in the order they started.
    pub sessions: Vec<SessionView>,
    /// The jobs running in the background.
    pub jobs: Vec<super::event::JobRow>,
    /// The id of the session shown.
    pub shown: String,
    /// The row chosen in the list while it has the keys, in the order the tree draws the
    /// sessions, then the jobs; `None` while the composer has them.
    pub focus: Option<usize>,
    /// The view changed: the screen is cleared before the next frame.
    pub replay: bool,
}

impl Default for Views {
    /// One lane, known by its id once it says it started.
    fn default() -> Self {
        Self {
            sessions: vec![SessionView::new(String::new(), None, String::new())],
            jobs: vec![],
            shown: String::new(),
            focus: None,
            replay: false,
        }
    }
}

/// One session as the interface keeps it: a lane, or an agent with its parent.
pub(crate) struct SessionView {
    pub id: String,
    /// The lane that started it, for an agent.
    pub parent: Option<String>,
    pub name: String,
    pub blocks: Vec<Vec<Line<'static>>>,
    /// Its calls running, in the order they started.
    pub running: Vec<Running>,
    /// When its task running started.
    pub busy: Option<Instant>,
    /// How long its finished tasks took, in milliseconds.
    pub worked: u64,
    /// The tokens its model turns used.
    pub tokens: u64,
    pub state: AgentState,
    /// A lane's task ended while it was not shown, and it has not been shown since.
    pub unseen: bool,
    /// How many tasks it has finished.
    pub tasks: usize,
    /// What only a lane has: the person's entries and what the task reads; `None` for an agent.
    pub lane: Option<Box<LaneState>>,
}

impl SessionView {
    pub(crate) fn new(id: String, parent: Option<String>, name: String) -> Self {
        let lane = parent.is_none().then(|| {
            Box::new(LaneState {
                mode: "default".into(),
                ..LaneState::default()
            })
        });
        Self {
            id,
            parent,
            name,
            blocks: vec![],
            running: vec![],
            busy: None,
            worked: 0,
            tokens: 0,
            state: AgentState::Idle,
            unseen: false,
            tasks: 0,
            lane,
        }
    }
}

/// Characters of a first task a lane is named by.
const NAMED: usize = 48;

impl SessionView {
    /// What the person knows it by: its name, else an explicitly new session.
    pub(crate) fn title(&self) -> &str {
        match self.name.is_empty() {
            true => "New session",
            false => &self.name,
        }
    }

    /// A lane with no name is named by the first line of its first task.
    /// Its task began: it works, and the transcript shows what it was asked (not the mode
    /// switch, which is no task of the person's).
    pub(crate) fn started(
        &mut self,
        goal: &str,
        now: Instant,
        width: usize,
    ) -> Option<Vec<Line<'static>>> {
        self.busy = Some(now);
        self.state = AgentState::Working;
        if let Some(lane) = &mut self.lane {
            lane.stopping = false;
        }
        if goal == protocol::SHIFT_TAB {
            return None;
        }
        self.named_by(goal);
        Some(super::cells::user(goal, width))
    }

    pub(crate) fn named_by(&mut self, goal: &str) {
        if self.lane.is_none() || !self.name.is_empty() {
            return;
        }
        let line = goal
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        self.name = super::wrap::cut(line.trim(), NAMED);
    }
}

/// A lane's own: what the person sent it, and what its task reports beside its transcript.
#[derive(Default)]
pub(crate) struct LaneState {
    /// The session waits for its next entry here.
    pub waiting: Option<SyncSender<Option<Typed>>>,
    /// Entries for after the task running (Tab, or a command), oldest first.
    pub queued: VecDeque<Typed>,
    /// What the person said to the task running, not yet read by it (Enter while it works).
    pub steered: Vec<Typed>,
    /// What came back from the background while nothing ran: tasks after what is queued.
    pub background: VecDeque<Typed>,
    /// The share of the context window left after the last task, in percent.
    pub context: Option<u8>,
    pub mode: String,
    /// The latest actions of the browser's hands under the call running.
    pub steps: Vec<String>,
    /// The person stopped the task running.
    pub stopping: bool,
    /// The person said no to a call, and what they said: for the call refused.
    pub declined: Option<String>,
    /// Its own choices, asked while it was not shown: they wait until it is.
    pub held: Vec<(super::select::Select, super::app::Open)>,
}

/// A call running: its id, what it is and how it reads, and when it started.
#[derive(Debug, Clone)]
pub(crate) struct Running {
    pub call: String,
    pub name: String,
    pub label: protocol::Label,
    pub since: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentState {
    /// Started, and given nothing yet.
    Idle,
    Working,
    /// Its last task ended so (`achieved`, `error`, …).
    Ended(String),
    Closed,
}

impl Views {
    pub(crate) fn find(&self, id: &str) -> Option<usize> {
        self.sessions.iter().position(|session| session.id == id)
    }

    /// A lane began: the interface's first takes its id (it is shown from the start); another
    /// is kept beside the others.
    pub(crate) fn lane_started(&mut self, id: String, name: String) {
        let first = &mut self.sessions[0];
        if first.id.is_empty() {
            first.id = id.clone();
            first.named_by(&name);
            self.shown = id;
        } else if self.find(&id).is_none() {
            let mut lane = SessionView::new(id, None, String::new());
            lane.named_by(&name);
            self.sessions.push(lane);
        }
    }

    /// The lane of the session shown: itself, or the agent's parent.
    pub(crate) fn home(&self) -> usize {
        let at = self.find(&self.shown).unwrap_or(0);
        match &self.sessions[at].parent {
            Some(parent) => self.find(parent).unwrap_or(0),
            None => at,
        }
    }

    /// The sessions the tree draws, in its order: the lane shown and the agents it started, or
    /// none while it has started none. The other lanes are the ← list's.
    pub(crate) fn order(&self) -> Vec<usize> {
        let lane = self.home();
        let id = &self.sessions[lane].id;
        let agents = (0..self.sessions.len())
            .filter(|&at| self.sessions[at].parent.as_ref() == Some(id))
            .collect::<Vec<_>>();
        match agents.is_empty() {
            true => vec![],
            false => std::iter::once(lane).chain(agents).collect(),
        }
    }
}
