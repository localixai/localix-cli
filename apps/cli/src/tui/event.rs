//! What reaches the interface: from the session (a task started, a call ran, a question waits)
//! and from the terminal (a key, a paste, a new size). Questions carry where their answer goes.

use std::sync::mpsc::Sender;

use contracts::person::{Answer, Question};
use contracts::tool::{ToolError, ToolOutput};
use crossterm::event::KeyEvent;
use serde_json::Value;

/// The answer to an approval.
#[derive(Debug, Clone, PartialEq)]
pub enum Approval {
    Yes,
    /// Yes, and the same tool is not asked about again.
    Always,
    /// No, and what the person said beside it.
    No(Option<String>),
}

pub enum Event {
    /// The session's first row: the version and what it runs with.
    Header(Vec<String>),
    /// A quiet line from llx; its rows are kept as they are.
    Note(String),
    Error(String),
    /// The mode changed: its name, `default` included.
    Mode(String),
    /// The session takes its next entry; `None` goes back when the person quits.
    Ready(Sender<Option<String>>),
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
        options: Vec<String>,
        reply: Sender<Option<usize>>,
    },
    /// The question open no longer waits (the task was stopped): it closes unanswered.
    Dismiss,
    Key(KeyEvent),
    Paste(String),
    Resize(u16, u16),
    /// The session is over: the terminal is given back.
    Quit,
}
