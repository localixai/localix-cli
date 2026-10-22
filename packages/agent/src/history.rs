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
    /// Where the last goal left its tool (its `Observation::location`): where a continued session
    /// resumes.
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub turns: Vec<Turn>,
}

/// One goal in a session, and how it ended.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub goal: String,
    /// The tool that drove it.
    #[serde(default)]
    pub tool: Option<String>,
    pub answer: String,
    pub status: String,
    /// Unix seconds.
    pub at: u64,
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
