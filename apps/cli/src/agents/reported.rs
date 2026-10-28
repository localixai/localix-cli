//! A tool as a script calls it: the same instance a direct call uses, and each call it makes
//! shown on its own (ARCHITECTURE.md X5). It reaches the log as a call named `exec › <tool>`,
//! and the trace and the interface as a line under the `exec` call running. `--json` has the
//! `exec` call alone: its events are a public contract.

use std::sync::atomic::{AtomicU64, Ordering};

use contracts::chat::ToolCall;
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::Value;

use crate::output::{Output, logged};
use crate::printable::one_line;
use crate::server::label::label;

/// What a script's calls are shown as: who made them and where they are reported.
pub(crate) struct Scene<'a> {
    pub output: &'a Output,
    /// Whom the log names: the session, or the agent's session.
    pub who: &'a str,
    /// The agent whose script it is, named in the line shown; none for the main session.
    pub agent: Option<&'a str>,
    /// Numbers the calls, for the log to tell them apart.
    pub made: &'a AtomicU64,
}

/// `inner`, reported to `scene` at each call.
pub(crate) struct Reported<'a> {
    pub inner: &'a dyn Tool,
    pub scene: &'a Scene<'a>,
}

impl Scene<'_> {
    fn show(&self, line: &str) {
        let line = match self.agent {
            Some(name) => format!("{name}: {line}"),
            None => line.to_string(),
        };
        self.output.substep(line);
    }
}

impl Tool for Reported<'_> {
    fn spec(&self) -> ToolSpec {
        self.inner.spec()
    }

    fn parallel(&self) -> bool {
        self.inner.parallel()
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let scene = self.scene;
        let name = self.inner.spec().name;
        let call = ToolCall {
            id: format!("exec-{}", scene.made.fetch_add(1, Ordering::SeqCst)),
            name: format!("exec › {name}"),
            arguments: arguments.clone(),
        };
        logged::call(scene.who, &call);
        let label = label(&name, arguments);
        scene.show(&format!("exec › {} {}", label.doing, label.subject));
        let outcome = self.inner.call(arguments);
        logged::result(scene.who, &call, &outcome);
        if let Err(ToolError::Failed(why) | ToolError::Fatal(why)) = &outcome {
            scene.show(&format!("exec › {name} failed: {}", one_line(why)));
        }
        outcome
    }
}

#[cfg(test)]
#[path = "reported_tests.rs"]
mod tests;
