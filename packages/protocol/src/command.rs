//! What the interface asks of the server, and what is done. A command names the session it is
//! for (W7): the server keeps no state per client but who is attached.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// The handshake, first and once (W3).
    Initialize { version: u32 },
    /// What the person sent a session: its next task, words for the task running, or a command.
    Input(Input),
    /// Ctrl-C: stop the session's task; asked again soon after, the person wants out.
    Interrupt { session: String },
    /// Stop what runs under `id` (a lane's task, an agent, a job), and never quit for it.
    Halt { id: String },
    /// Open a lane, or show it when it is open already.
    Open { target: Target },
    /// This directory's saved sessions, for the list.
    List,
    /// "Quit anyway": every lane is stopped, then the work ends.
    QuitAll,
    /// The person left: what waits for them ends with nothing.
    Leave,
    /// What still runs, for the question asked before quitting.
    Work,
}

/// What the person sent: the text, and the pictures attached to it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    pub session: String,
    pub text: String,
    pub images: Vec<PathBuf>,
    /// For after the task running (Tab), not for it.
    pub later: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "id", rename_all = "snake_case")]
pub enum Target {
    /// A new session.
    New,
    /// A session saved before, by its id.
    Saved(String),
}

/// What came of a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Done {
    Ok,
    /// The server and what it allows.
    Initialized {
        version: u32,
        max_lanes: usize,
    },
    /// The task was stopped; `again` when it was asked a second time in a row.
    Interrupted {
        again: bool,
    },
    /// Terminal sessions running, in every lane.
    Work {
        shells: usize,
    },
    /// The command could not be done, and why.
    Failed {
        why: String,
    },
}
