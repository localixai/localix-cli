//! What reaches the interface: from the session (a task started, a call ran, a question waits)
//! and from the terminal (a key, a paste, a new size). Questions carry where their answer goes.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use contracts::person::{Answer, Question};
use contracts::tool::{ToolError, ToolOutput};
use crossterm::event::KeyEvent;
use serde_json::Value;

/// The answer to an approval.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Approval {
    Yes,
    /// Yes, and the same tool is not asked about again.
    Always,
    /// No, and what the person said beside it.
    No(Option<String>),
}

/// What the person sent: the text, and the pictures attached to it.
#[derive(Debug, Clone, Default, PartialEq)]
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

/// A job as the list under the composer shows it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JobRow {
    pub id: String,
    pub name: String,
    /// Its command, or its cron and prompt.
    pub detail: String,
    /// A monitor's deadline.
    pub until: Option<std::time::Instant>,
    /// A schedule's next run, Unix seconds.
    pub next: Option<u64>,
    /// Batches a monitor gave.
    pub events: usize,
    /// The session whose job it is (set as it arrives): its stop is known under it.
    pub lane: String,
}

pub(crate) enum Event {
    /// The session's first row: the version and what it runs with.
    Header(Vec<String>),
    /// A quiet line from llx; its rows are kept as they are.
    Note(String),
    Error(String),
    /// The mode changed: its name, `default` included.
    Mode(String),
    /// The session takes its next entry; `None` goes back when the person quits.
    Ready(Sender<Option<Typed>>),
    TaskStarted(String),
    Call {
        name: String,
        args: Value,
    },
    Result {
        name: String,
        args: Value,
        outcome: Result<ToolOutput, ToolError>,
    },
    /// One action of the browser's hands, under the call that runs them.
    Step(String),
    /// The model's answer.
    Answer(String),
    TaskEnded {
        status: String,
        millis: u64,
        turns: u64,
        cost: f64,
    },
    Approve {
        tool: String,
        summary: String,
        reply: Sender<Approval>,
    },
    /// One question of the `ask` tool: the `at`th of `of`.
    Ask {
        question: Question,
        at: usize,
        of: usize,
        reply: Sender<Option<Answer>>,
    },
    /// A choice of llx's own (carry out the plan?): a title and its options, and the one picked.
    Choose {
        title: String,
        /// Each option's label and a short note.
        options: Vec<(String, String)>,
        /// The option in effect now, where the marker starts.
        marked: Option<usize>,
        reply: Sender<Option<usize>>,
    },
    /// A choice with a value adjusted inline for each row.
    ChooseAdjustable {
        title: String,
        options: Vec<(String, String)>,
        marked: Option<usize>,
        adjustments: Vec<crate::picker::Adjustment>,
        reply: Sender<Option<(usize, usize)>>,
    },
    /// A line to type (a key, a pasted address), hidden when it is a secret.
    Line {
        prompt: String,
        hidden: bool,
        reply: Sender<Option<String>>,
    },
    /// The question open no longer waits (the task was stopped): it closes unanswered.
    Dismiss,
    /// A resumed session's tasks so far, oldest first: shown above what it does next.
    Earlier(Vec<contracts::history::Turn>),
    /// What is known of the session changed (sent inside its [`Event::In`]): its title.
    Updated(crate::meta::Update),
    /// A session of the interface began: its stop is known by its id.
    LaneStarted {
        id: String,
        name: String,
    },
    /// This directory's saved sessions, newest first: each id, its first task, and when it ran
    /// and how many tasks it has (`2 h ago · 4 tasks`).
    Saved(Vec<(String, String, String)>),
    /// Show lane `id`: the desk opened it, or it was asked for and is open already.
    Show(String),
    /// What happened in session `id` (a lane, or an agent): shown in its view.
    In(String, Box<Event>),
    /// The session started an agent (sent inside its parent's [`Event::In`]).
    Started {
        id: String,
        name: String,
    },
    /// The agent was closed (sent inside its own [`Event::In`]): it stays in the tree, dimmed.
    Closed,
    /// Tokens a model turn of an agent used (sent inside [`Event::In`]).
    Spent(u64),
    /// The lane's conversation went back or began again: its agents are gone.
    AgentsCleared,
    /// What came back from the background (a monitor's lines, a schedule due) while nothing
    /// runs: a task of its own, after what the person queued.
    Background(String),
    /// Words put in the composer for the person to change and send (a rewound task).
    Prefill(String),
    /// The share of the model's context window left, in percent, after a task.
    Context(u8),
    /// The jobs running in the background, all of them, whenever they change.
    Jobs(Vec<JobRow>),
    /// The task running reads what the person said to it meanwhile (taken from the interface,
    /// and shown as theirs), or only looks whether there is any (`peek`).
    TakeSteered {
        reply: Sender<Vec<String>>,
        peek: bool,
    },
    Key(KeyEvent),
    Paste(String),
    Resize(u16, u16),
    /// The session is over: the terminal is given back.
    Quit,
}
