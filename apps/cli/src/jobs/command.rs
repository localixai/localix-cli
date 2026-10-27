//! One background command, using the same approval and runner as `bash`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use contracts::tool::{ToolError, ToolOutput};
use serde_json::json;
use tools::shell::{BashTool, Sessions};

use super::tool::{JobsTool, new_job};
use super::{Job, What};

impl JobsTool<'_, '_> {
    pub(super) fn command(
        &self,
        command: String,
        workdir: Option<&str>,
        timeout_ms: Option<u64>,
        name: Option<String>,
    ) -> Result<ToolOutput, ToolError> {
        let sessions = Sessions::default();
        let stopped = || false;
        let bash = BashTool {
            workspace: self.kit.workspace,
            gate: self.kit.gate,
            hidden: self.kit.hidden,
            stop: &stopped,
            sessions: &sessions,
        };
        let args = json!({"command": command, "workdir": workdir, "timeout_ms": timeout_ms});
        let approved = bash.approve_command(&args)?;
        let stop = Arc::new(AtomicBool::new(false));
        let (id, name) = {
            let mut state = self.jobs.lock();
            if state.jobs.len() >= 50 {
                return Err(ToolError::Failed("50 jobs are kept: stop one first".into()));
            }
            let (id, name) = new_job(&mut state, name);
            state.jobs.push(Job {
                id: id.clone(),
                name: name.clone(),
                what: What::Command {
                    command,
                    until: Instant::now()
                        + Duration::from_millis(timeout_ms.unwrap_or(120_000).clamp(1, 600_000)),
                },
                stop: Arc::clone(&stop),
                events: 0,
                _known: Some(self.jobs.known(&id, &stop)),
            });
            (id, name)
        };
        self.jobs.show(self.kit);
        let (kit, jobs) = (self.kit, self.jobs);
        let (ours, label) = (id.clone(), name.clone());
        let started = std::thread::Builder::new()
            .name("llx-command-job".into())
            .spawn_scoped(self.scope, move || {
                let sessions = Sessions::default();
                let stopped = || stop.load(Ordering::SeqCst);
                let bash = BashTool {
                    workspace: kit.workspace,
                    gate: kit.gate,
                    hidden: kit.hidden,
                    stop: &stopped,
                    sessions: &sessions,
                };
                let result = match bash.run_approved(approved) {
                    Ok(output) => output.report,
                    Err(error) => error.to_string(),
                };
                let listed = jobs.lock().jobs.iter().any(|job| job.id == id);
                if listed && !jobs.over.load(Ordering::SeqCst) {
                    kit.queue.post(crate::arrivals::job_result(&label, &result));
                }
                jobs.lock().jobs.retain(|job| job.id != id);
                jobs.show(kit);
            });
        if let Err(error) = started {
            self.jobs.lock().jobs.retain(|job| job.id != ours);
            self.jobs.show(self.kit);
            return Err(ToolError::Failed(format!(
                "the command could not start (no thread: {error})"
            )));
        }
        Ok(ToolOutput::from(format!(
            "Command `{name}` ({ours}) started; its result will arrive as <job_result>."
        )))
    }
}
