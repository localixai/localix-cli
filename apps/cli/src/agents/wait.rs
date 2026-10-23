//! The `agents` tool's `wait`, and how it finds the agent a call names.

use std::time::{Duration, Instant};

use contracts::tool::{ToolError, ToolOutput};

use super::tool::AgentsTool;
use super::{POLL, spec};

impl AgentsTool<'_, '_> {
    /// Wait for one of `agents` (any agent working, when none is named) to answer.
    pub(super) fn wait(
        &self,
        agents: &[String],
        timeout: Duration,
    ) -> Result<ToolOutput, ToolError> {
        let named: Vec<String> = agents
            .iter()
            .map(|agent| self.find(agent).map(|at| self.id_at(at)))
            .collect::<Result<_, _>>()?;
        let ui = self.kit.output.ui();
        let until = Instant::now() + timeout;
        loop {
            let mut state = self.registry.lock();
            let targets: Vec<String> = match named.is_empty() {
                true => state
                    .children
                    .iter()
                    .filter(|c| c.working)
                    .map(|c| c.session.id.clone())
                    .chain(state.mail.iter().map(|(id, _)| id.clone()))
                    .collect(),
                false => named.clone(),
            };
            let mut answers = vec![];
            state.mail.retain(|(id, message)| {
                let wanted = targets.contains(id);
                if wanted {
                    answers.push(message.clone());
                }
                !wanted
            });
            let working: Vec<String> = state
                .children
                .iter()
                .filter(|c| c.working)
                .map(|c| format!("`{}`", c.session.name))
                .collect();
            let still = match working.is_empty() {
                true => String::new(),
                false => format!("\n\nStill working: {}.", working.join(", ")),
            };
            if !answers.is_empty() {
                return Ok(ToolOutput::from(format!("{}{still}", answers.join("\n\n"))));
            }
            let busy = state
                .children
                .iter()
                .any(|c| c.working && targets.contains(&c.session.id));
            if !busy {
                return Ok(ToolOutput::from(format!(
                    "No agent you wait for is working, and none has an answer waiting: an answer \
                     that came already is above, as an `<agent_result>` message.{still}"
                )));
            }
            drop(state);
            if (self.kit.stopped)() {
                return Err(ToolError::Failed(crate::gate::STOPPED.into()));
            }
            if ui.is_some_and(|ui| !ui.steered(true).is_empty()) {
                return Ok(ToolOutput::from(format!(
                    "The wait ended: the person wrote to you. Read it, then wait again if you \
                     still need to.{still}"
                )));
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(ToolOutput::from(format!(
                    "Timed out after {} s with no answer.{still}",
                    timeout.as_secs()
                )));
            }
            self.registry.wait_change(left.min(POLL));
        }
    }

    fn id_at(&self, at: usize) -> String {
        self.registry.lock().children[at].session.id.clone()
    }

    /// The agent `agent` names: its name, or its session id.
    pub(super) fn find(&self, agent: &str) -> Result<usize, ToolError> {
        let state = self.registry.lock();
        let wanted = spec::tidy(agent).unwrap_or_default();
        let at = state
            .children
            .iter()
            .position(|child| child.session.id == agent.trim() || child.session.name == wanted);
        at.ok_or_else(|| {
            let names: Vec<String> = state
                .children
                .iter()
                .map(|child| format!("`{}`", child.session.name))
                .collect();
            ToolError::Failed(match names.is_empty() {
                true => format!(
                    "there is no agent `{agent}`: none was started in this session (an agent \
                     does not outlive the session it was started in)"
                ),
                false => format!(
                    "there is no agent `{agent}`; there are {}",
                    names.join(", ")
                ),
            })
        })
    }
}
