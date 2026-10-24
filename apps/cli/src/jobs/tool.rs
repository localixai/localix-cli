//! The `jobs` tool: the model starts a monitor or a schedule, lists them and stops them.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::Scope;
use std::time::{Duration, Instant};

use contracts::gate::{Effect, Verdict};
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::Value;

use super::monitor::{self, Ended};
use super::spec::{self, Action, When};
use super::{Job, Jobs, Kit, State, What, cron};

/// Monitors running at once, and schedules kept, at most.
const MAX_MONITORS: usize = 8;
const MAX_SCHEDULES: usize = 50;
/// How long a recurring schedule lives.
const WEEK: u64 = 7 * 86_400;
/// Batches a monitor may give before it is stopped as a firehose.
const MAX_EVENTS: usize = 100;
/// A watch's deadline at most in `exec`, where nobody is waiting.
const EXEC_WATCH: Duration = Duration::from_secs(600);

pub(crate) struct JobsTool<'scope, 'env> {
    kit: &'env Kit<'env>,
    jobs: &'env Jobs,
    scope: &'scope Scope<'scope, 'env>,
}

impl<'scope, 'env> JobsTool<'scope, 'env> {
    pub(crate) fn new(
        kit: &'env Kit<'env>,
        jobs: &'env Jobs,
        scope: &'scope Scope<'scope, 'env>,
    ) -> Self {
        Self { kit, jobs, scope }
    }

    fn ask(&self, summary: &str) -> Result<(), ToolError> {
        match self.kit.gate.check(Effect {
            tool: spec::NAME,
            summary,
        }) {
            Verdict::Allow => Ok(()),
            Verdict::Deny(why) => Err(ToolError::Failed(why)),
        }
    }

    fn monitor(
        &self,
        command: &str,
        name: Option<String>,
        timeout: Duration,
    ) -> Result<ToolOutput, ToolError> {
        let dir = self.kit.workspace.root();
        self.ask(&format!("monitor: {command}  (in {})", dir.display()))?;
        let timeout = match self.kit.session {
            true => timeout,
            false => timeout.min(EXEC_WATCH),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let (id, name) = {
            let mut state = self.jobs.lock();
            let running = state
                .jobs
                .iter()
                .filter(|job| matches!(job.what, What::Monitor { .. }))
                .count();
            if running >= MAX_MONITORS {
                return Err(ToolError::Failed(format!(
                    "{running} monitors run already: stop one first"
                )));
            }
            let (id, name) = new_job(&mut state, name);
            state.jobs.push(Job {
                id: id.clone(),
                name: name.clone(),
                what: What::Monitor {
                    command: command.to_string(),
                    until: Instant::now() + timeout,
                },
                stop: Arc::clone(&stop),
                events: 0,
                _known: Some(self.jobs.known(&id, &stop)),
            });
            (id, name)
        };
        self.jobs.show(self.kit);
        let (kit, jobs) = (self.kit, self.jobs);
        let (command, label) = (command.to_string(), name.clone());
        self.scope.spawn(move || {
            let lines = |lines: Vec<String>, dropped: usize| {
                let mut text = lines.join("\n");
                if dropped > 0 {
                    text.push_str(&format!("\n… {dropped} more lines"));
                }
                // A job no longer listed (`/new` let it go) says nothing more; a new one may
                // have its name, never its id.
                let mut state = jobs.lock();
                let Some(job) = state.jobs.iter_mut().find(|job| job.id == id) else {
                    return;
                };
                job.events += 1;
                if job.events >= MAX_EVENTS {
                    job.stop.store(true, Ordering::SeqCst);
                    text.push_str(&format!(
                        "\n(stopped after {MAX_EVENTS} batches: start it again with a \
                         filter that prints only the lines you act on)"
                    ));
                }
                drop(state);
                kit.queue.post(crate::arrivals::monitor(&label, &text));
                jobs.show(kit);
            };
            let stopped = || stop.load(Ordering::SeqCst);
            let root = kit.workspace.root();
            let deadline = Instant::now() + timeout;
            let names = (kit.hidden)();
            let hidden: Vec<&str> = names.iter().map(String::as_str).collect();
            let env = tools::shell::Env {
                vars: kit.workspace.env(),
                hidden: &hidden,
            };
            let ended = monitor::watch(&command, root, env, deadline, &stopped, &lines);
            let listed = {
                let mut state = jobs.lock();
                let before = state.jobs.len();
                state.jobs.retain(|job| job.id != id);
                state.jobs.len() < before
            };
            let why = match ended {
                Ok(Ended::Exit(how)) => format!("its command ended ({how})"),
                Ok(Ended::Deadline) => {
                    "its deadline passed; start it again if it is still needed".into()
                }
                Ok(Ended::Stopped) => "it was stopped".into(),
                Err(why) => why,
            };
            if listed && !jobs.over.load(Ordering::SeqCst) {
                let ended = format!("The monitor ended: {why}.");
                kit.queue.post(crate::arrivals::monitor(&label, &ended));
            }
            jobs.show(kit);
        });
        Ok(ToolOutput::from(format!(
            "Monitor `{name}` started: each line it prints comes to you as a `<monitor>` message. \
             It ends in {} min at the latest.",
            timeout.as_secs().div_ceil(60)
        )))
    }

    fn schedule(
        &self,
        prompt: &str,
        when: When,
        recurring: bool,
        name: Option<String>,
    ) -> Result<ToolOutput, ToolError> {
        if !self.kit.session {
            return Err(ToolError::Failed(
                "schedules need a session: this run ends with its task".into(),
            ));
        }
        let now = session::now();
        let (cron, next, said) = match when {
            When::Cron(cron) => {
                let next = cron
                    .next(now)
                    .ok_or_else(|| ToolError::Failed("it never runs within a year".into()))?;
                let said = format!(
                    "{} ({})",
                    cron.text(),
                    if recurring { "recurring" } else { "once" }
                );
                (recurring.then_some(cron), next, said)
            }
            When::In(seconds) => (None, now + seconds, format!("in {} min", seconds / 60)),
        };
        self.ask(&format!("schedule {said}: {prompt}"))?;
        let (id, name) = {
            let mut state = self.jobs.lock();
            let kept = state.jobs.len();
            if kept >= MAX_SCHEDULES {
                return Err(ToolError::Failed(format!(
                    "{kept} jobs are kept: stop one first"
                )));
            }
            let (id, name) = new_job(&mut state, name);
            let stop = Arc::new(AtomicBool::new(false));
            let known = Some(self.jobs.known(&id, &stop));
            state.jobs.push(Job {
                id: id.clone(),
                name: name.clone(),
                what: What::Schedule {
                    prompt: prompt.to_string(),
                    cron,
                    next,
                    expires: now + WEEK,
                },
                stop,
                events: 0,
                _known: known,
            });
            (id, name)
        };
        // The scheduler waits for the soonest it knew of: this one may be sooner.
        self.jobs.bell.ring();
        self.jobs.show(self.kit);
        Ok(ToolOutput::from(format!(
            "Scheduled `{name}` ({id}), {said}: next at {}. It comes to you as a task of its own, \
             between tasks.",
            cron::clock(next)
        )))
    }

    fn list(&self) -> ToolOutput {
        let state = self.jobs.lock();
        if state.jobs.is_empty() {
            return ToolOutput::from("Nothing runs in the background.".to_string());
        }
        let rows: Vec<String> = state.jobs.iter().map(super::line).collect();
        ToolOutput::from(rows.join("\n"))
    }

    fn stop(&self, wanted: &str) -> Result<ToolOutput, ToolError> {
        let mut state = self.jobs.lock();
        let at = state
            .jobs
            .iter()
            .position(|job| job.id == wanted || job.name == wanted)
            .ok_or_else(|| ToolError::Failed(format!("there is no job `{wanted}`")))?;
        let job = &state.jobs[at];
        job.stop.store(true, Ordering::SeqCst);
        let name = job.name.clone();
        if matches!(job.what, What::Schedule { .. }) {
            state.jobs.remove(at);
        }
        drop(state);
        self.jobs.show(self.kit);
        Ok(ToolOutput::from(format!("Stopped `{name}`.")))
    }
}

impl Tool for JobsTool<'_, '_> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: spec::NAME.into(),
            description: spec::description(),
            parameters: spec::parameters(),
        }
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        match spec::action(arguments).map_err(ToolError::Failed)? {
            Action::Monitor {
                command,
                name,
                timeout,
            } => self.monitor(&command, name, timeout),
            Action::Schedule {
                prompt,
                when,
                recurring,
                name,
            } => self.schedule(&prompt, when, recurring, name),
            Action::List => Ok(self.list()),
            Action::Stop { job } => self.stop(&job),
        }
    }
}

/// A new job's id and name (the name given, else its id).
fn new_job(state: &mut State, name: Option<String>) -> (String, String) {
    state.made += 1;
    let id = format!("job-{}", state.made);
    let name = name
        .filter(|name| state.jobs.iter().all(|job| job.name != *name))
        .unwrap_or_else(|| id.clone());
    (id, name)
}
