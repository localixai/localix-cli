//! The tools, one module each. A tool implements the agent's tool protocol
//! (`contracts::tool::Tool`); it knows nothing of the agent or the models. A tool with
//! an effect outside the conversation (a changed file, a command) asks a `contracts::gate::Gate`
//! first.
//! - [`workspace`]: the directory the coding tools work in: where paths resolve, where files may
//!   change.
//! - [`fs`]: `read` (a file's numbered lines, a directory's entries) and `edit` (exact replacement,
//!   or a new file).
//! - [`shell`]: `bash`: a command run in the workspace, its exit code and its output.
//! - [`ask`]: questions to the person (`contracts::person::Person`), who answers in the app.

use contracts::gate::{Effect, Gate, Verdict};
use contracts::tool::ToolError;

pub mod ask;
pub mod fs;
pub mod shell;
pub mod workspace;

#[cfg(test)]
mod testing;

/// Ask `gate` before an effect on `subject` ([`Effect::subject`]); a refusal is a failed call the
/// model reads, so it can try another way.
fn ask(gate: &dyn Gate, tool: &str, subject: &str, summary: &str) -> Result<(), ToolError> {
    match gate.check(Effect {
        tool,
        subject,
        summary,
    }) {
        Verdict::Allow => Ok(()),
        Verdict::Deny(reason) => Err(ToolError::Failed(format!("not allowed: {reason}"))),
    }
}

/// `mutex`, locked even if a holder panicked: what it guards is plain data, still usable, and a
/// tool fails its call rather than the process.
fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A call's `arguments` read as the tool's own `T`; what does not fit is a failed call the model
/// reads.
fn args<T: serde::de::DeserializeOwned>(arguments: &serde_json::Value) -> Result<T, ToolError> {
    serde_json::from_value(arguments.clone())
        .map_err(|error| ToolError::Failed(format!("unusable arguments: {error}")))
}
