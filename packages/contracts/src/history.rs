//! The history port: sessions of goals, kept by whatever store the app composes. A session is the
//! goals asked one after another with the same tools, how each ended, and where the last one left
//! its tool, so it can be listed and continued.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    /// Unix seconds.
    pub created: u64,
    pub updated: u64,
    /// Where its goals ran, as the app names it (a directory): a session continued elsewhere is
    /// told apart from one continued in place. Empty in sessions saved before it was kept.
    #[serde(default)]
    pub workspace: String,
    /// Where the last goal left its tool (its `Observation::location`): where a continued session
    /// resumes.
    #[serde(default)]
    pub location: String,
    /// The session that started this one, when it is an agent another session's agent started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// An agent's short name, as its parent calls it; empty for a session the person started.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// What the session is about, as a title made from its first task; empty until one is.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default)]
    pub turns: Vec<Turn>,
}

/// One goal in a session, and how it ended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub goal: String,
    pub answer: String,
    pub status: String,
    /// Unix seconds.
    pub at: u64,
    /// What the task did, in the order it began: each call and each model turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
}

/// One step of a task: a tool call or a model turn, when it began and how long it took.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// The tool's name, or `model` for a model turn.
    pub what: String,
    /// What the call did, in a few words (`open w1 https://…`), or a turn's tokens.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
    /// Milliseconds from the task's start.
    pub at_ms: u64,
    pub ms: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub failed: bool,
}

impl Session {
    /// Record a finished goal, at `at` (Unix seconds).
    pub fn record(&mut self, turn: Turn, location: Option<&str>) {
        self.updated = turn.at;
        if let Some(location) = location {
            self.location = location.to_string();
        }
        self.turns.push(turn);
    }

    /// Whether another session started this one.
    pub fn is_agent(&self) -> bool {
        self.parent.is_some()
    }
}

/// Where sessions are kept.
pub trait History {
    /// A new, empty session with a fresh id.
    fn start(&self) -> Session;
    fn save(&self, session: &Session) -> Result<(), String>;
    /// The session with `id`, or the newest one for `None`.
    fn load(&self, id: Option<&str>) -> Result<Session, String>;
    /// Every session, newest first.
    fn list(&self) -> Vec<Session>;
}
