//! The `agents` tool: `spawn`, `send`, `list` and `close`, each an action on the registry.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::Scope;

use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::Value;

use super::kit::Kit;
use super::spec::{self, Action};
use super::{Child, MAX_OPEN, Registry, child, unique};
use crate::tui::event::Event;

pub struct AgentsTool<'scope, 'env> {
    pub(super) kit: &'env Kit<'env>,
    pub(super) registry: &'env Registry,
    scope: &'scope Scope<'scope, 'env>,
}

impl<'scope, 'env> AgentsTool<'scope, 'env> {
    pub fn new(
        kit: &'env Kit<'env>,
        registry: &'env Registry,
        scope: &'scope Scope<'scope, 'env>,
    ) -> Self {
        Self {
            kit,
            registry,
            scope,
        }
    }

    fn spawn(&self, task: &str, name: Option<String>) -> Result<ToolOutput, ToolError> {
        let (id, name) = {
            let mut state = self.registry.lock();
            let open = state.children.iter().filter(|child| child.open).count();
            if open >= MAX_OPEN {
                return Err(ToolError::Failed(format!(
                    "{open} agents are open: close one you no longer need first"
                )));
            }
            let name = unique(name, &state.children);
            let mut session = self.kit.history.start();
            session.parent = Some(state.parent.clone());
            session.name = name.clone();
            session.workspace = self.kit.root().display().to_string();
            if let Some(ui) = self.kit.output.ui() {
                ui.send(Event::Started {
                    id: session.id.clone(),
                    name: name.clone(),
                });
            }
            let stop = Arc::new(AtomicBool::new(false));
            let known = crate::interrupt::register_flag(&session.id, stop.clone());
            let id = session.id.clone();
            state.children.push(Child {
                session,
                conversation: None,
                seen: crate::instructions::chain(self.kit.workspace.root())
                    .into_iter()
                    .collect(),
                told: self.kit.started,
                status: String::new(),
                open: true,
                working: false,
                stop,
                _known: known,
                letters: Arc::default(),
            });
            (id, name)
        };
        self.start(&id, task);
        Ok(ToolOutput::from(format!(
            "Started `{name}` (session {id}): it works in the background. Its answer comes to \
             you by itself once it is done; go on with other work, or `wait` when you need it."
        )))
    }

    /// Give the agent with session `id` its next task, on a thread of its own.
    pub(super) fn start(&self, id: &str, task: &str) {
        let mut state = self.registry.lock();
        if let Some(child) = state.children.iter_mut().find(|c| c.session.id == id) {
            child.working = true;
            child.stop.store(false, Ordering::SeqCst);
        }
        drop(state);
        let (kit, registry) = (self.kit, self.registry);
        let (id, task) = (id.to_string(), task.to_string());
        self.scope
            .spawn(move || child::work(kit, registry, &id, &task));
    }

    fn send(&self, agent: &str, message: &str) -> Result<ToolOutput, ToolError> {
        let at = self.find(agent)?;
        let state = self.registry.lock();
        let child = &state.children[at];
        let name = child.session.name.clone();
        if !child.open {
            return Err(ToolError::Failed(format!(
                "`{name}` is closed: spawn a new agent, and give it what it needs"
            )));
        }
        if child.working {
            child.letters.post(message);
            return Ok(ToolOutput::from(format!(
                "Sent to `{name}`: it reads it at its next step. Its answer comes to you as \
                 before."
            )));
        }
        let id = child.session.id.clone();
        drop(state);
        self.start(&id, message);
        Ok(ToolOutput::from(format!(
            "`{name}` goes on with it in the background; its answer comes to you once it is done."
        )))
    }

    fn list(&self) -> ToolOutput {
        let state = self.registry.lock();
        if state.children.is_empty() {
            return ToolOutput::from("No agents in this session yet.".to_string());
        }
        let rows: Vec<String> = state
            .children
            .iter()
            .map(|child| {
                let tasks = child.session.turns.len() + usize::from(child.working);
                let now = match (child.open, child.working) {
                    (false, _) => "closed".to_string(),
                    (true, true) => "working".to_string(),
                    (true, false) => {
                        format!("idle, the last task {}", child.status.replace('_', " "))
                    }
                };
                let tasks = crate::tui::cells::count(tasks as u64, "task");
                format!(
                    "- `{}` (session {}): {now}, {tasks}",
                    child.session.name, child.session.id
                )
            })
            .collect();
        ToolOutput::from(rows.join("\n"))
    }

    fn close(&self, agent: &str) -> Result<ToolOutput, ToolError> {
        let at = self.find(agent)?;
        let mut state = self.registry.lock();
        let child = &mut state.children[at];
        child.open = false;
        child.stop.store(true, Ordering::SeqCst);
        if !child.working {
            child.conversation = None;
        }
        if let Some(ui) = self.kit.output.ui() {
            ui.send(Event::In(child.session.id.clone(), Box::new(Event::Closed)));
        }
        let (name, id) = (child.session.name.clone(), child.session.id.clone());
        (self.registry.queue).forget(|entry| entry.key.as_deref() == Some(id.as_str()));
        Ok(ToolOutput::from(format!(
            "Closed `{name}`; its session {id} stays saved."
        )))
    }
}

impl Tool for AgentsTool<'_, '_> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: spec::NAME.into(),
            description: spec::description(),
            parameters: spec::parameters(),
        }
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        match spec::action(arguments).map_err(ToolError::Failed)? {
            Action::Spawn { task, name } => self.spawn(&task, name),
            Action::Send { agent, message } => self.send(&agent, &message),
            Action::Wait { agents, timeout } => self.wait(&agents, timeout),
            Action::List => Ok(self.list()),
            Action::Close { agent } => self.close(&agent),
        }
    }
}
