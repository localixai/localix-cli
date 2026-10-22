//! What a run shows as it goes: each tool call, what it came to, and what each model turn cost.
//! The agent reports and never waits on the answer, so a live trace, a progress line or a test
//! can watch a run without changing it.

use contracts::chat::{ToolCall, Usage};
use contracts::tool::{ToolError, ToolOutput};

/// Watches a run. Every event has a default that does nothing, so a watcher implements only the
/// ones it shows.
pub trait Observer {
    /// The model asked for `call`; it is about to be carried out.
    fn call(&self, _call: &ToolCall) {}
    /// What `call` came to, as the model reads it: every call gets one, including a `result` that
    /// breaks its schema, an unknown tool and a call that ends the run.
    fn result(&self, _call: &ToolCall, _outcome: &Result<ToolOutput, ToolError>) {}
    /// One model turn ended, and what it took.
    fn turn(&self, _usage: &Usage) {}
}

/// Shows nothing: `--json`, quiet runs and tests.
pub struct Silent;

impl Observer for Silent {}
