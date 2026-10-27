//! What an agent's tools are built from, and the tools built: the main session's, and each
//! agent's own, on a gate that names it and a stop of its own (the task's, or the person's `x` on
//! its row); each learns a subtree's instructions on its own.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use contracts::chat::Chat;
use contracts::gate::Gate;
use contracts::history::History;
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::Value;
use tools::fs::{EditTool, ReadTool};
use tools::shell::BashTool;
use tools::workspace::Workspace;

use crate::instructions::Instructed;
use crate::mode::{Mode, ModeCell};
use crate::output::Output;

/// The main session's parts an agent works with.
pub(crate) struct Kit<'a> {
    pub chat: &'a dyn Chat,
    pub workspace: &'a Workspace,
    /// The main session's gate (the mode, the person, the stop).
    pub gate: &'a dyn Gate,
    /// What commands never see, read at each command.
    pub hidden: &'a (dyn Fn() -> Vec<String> + Sync),
    /// Whether the task was stopped.
    pub stopped: &'a (dyn Fn() -> bool + Sync),
    /// The context with no earlier session: an agent knows nothing of the parent's history.
    pub context: &'a str,
    pub max_turns: usize,
    /// The model's context window, set at each task (the model can change between them).
    pub window: &'a std::sync::atomic::AtomicU64,
    /// How each agent's conversation is prepared for the model.
    pub strategy: &'a dyn agent::history::Strategy,
    /// Where an agent's file changes are kept too, for `/rewind`.
    pub checkpoints: &'a crate::checkpoints::Checkpoints,
    pub output: &'a Output,
    pub history: &'a dyn History,
    pub mode: &'a ModeCell,
    /// The mode the context states.
    pub started: Mode,
    /// The command line's rules, beside each mode's.
    pub rules: crate::rules::Rules,
    pub terminal: bool,
    /// The tools the main session offers, as its last task offered them: an agent is offered the
    /// same, so its requests start as the main session's do and read the provider's prompt cache
    /// from there.
    pub main: Mutex<Vec<ToolSpec>>,
}

impl Kit<'_> {
    pub(crate) fn root(&self) -> PathBuf {
        self.workspace.root().to_path_buf()
    }
}

/// The parts an agent's tools borrow, made in its thread.
pub(crate) struct Own<'a> {
    pub gate: &'a dyn Gate,
    pub stop: &'a (dyn Fn() -> bool + Sync),
    pub seen: Mutex<HashSet<PathBuf>>,
}

/// A session's tools: `read`, `edit` and `bash` (commands run to their end).
pub(crate) struct Tools<'a> {
    kit: &'a Kit<'a>,
    own: &'a Own<'a>,
    read: ReadTool<'a>,
    edit: EditTool<'a>,
    bash: BashTool<'a>,
}

impl<'a> Tools<'a> {
    pub(crate) fn new(kit: &'a Kit<'a>, own: &'a Own<'a>) -> Self {
        Self {
            kit,
            own,
            read: ReadTool {
                workspace: kit.workspace,
            },
            edit: EditTool {
                workspace: kit.workspace,
                gate: own.gate,
            },
            bash: BashTool {
                workspace: kit.workspace,
                gate: own.gate,
                hidden: kit.hidden,
                stop: own.stop,
            },
        }
    }

    /// Run `work` with the tools as the agent offers them.
    pub(crate) fn with<T>(&self, work: impl FnOnce(&[&dyn Tool]) -> T) -> T {
        let (kit, own) = (self.kit, self.own);
        let read = Instructed {
            inner: &self.read,
            root: kit.root(),
            seen: &own.seen,
        };
        let tracked = crate::checkpoints::Tracked {
            inner: &self.edit,
            workspace: kit.workspace,
            checkpoints: kit.checkpoints,
        };
        let edit = Instructed {
            inner: &tracked,
            root: kit.root(),
            seen: &own.seen,
        };
        let tools: [&dyn Tool; 3] = [&read, &edit, &self.bash];
        work(&tools)
    }

    /// Run `work` with the tools as the main session offers them, in its order: an agent's own
    /// where it has one, the rest withheld. Before the main session's first task, its own alone.
    pub(crate) fn as_main<T>(&self, work: impl FnOnce(&[&dyn Tool]) -> T) -> T {
        let main = crate::sync::lock(&self.kit.main).clone();
        self.with(|own| {
            let withheld: Vec<Withheld> = (main.iter())
                .filter(|spec| !own.iter().any(|tool| tool.spec().name == spec.name))
                .cloned()
                .map(Withheld)
                .collect();
            let offered = main.iter().filter_map(|spec| {
                let mine = own
                    .iter()
                    .find(|tool| tool.spec().name == spec.name)
                    .copied();
                mine.or_else(|| {
                    (withheld.iter())
                        .find(|kept| kept.0.name == spec.name)
                        .map(|kept| kept as &dyn Tool)
                })
            });
            let tools: Vec<&dyn Tool> = match main.is_empty() {
                true => own.to_vec(),
                false => offered.collect(),
            };
            work(&tools)
        })
    }
}

/// A tool of the main session an agent is offered but may not use (`agents`, `ask`, `jobs`):
/// offered the same, refused when called.
struct Withheld(ToolSpec);

impl Tool for Withheld {
    fn spec(&self) -> ToolSpec {
        self.0.clone()
    }

    fn call(&self, _: &Value) -> Result<ToolOutput, ToolError> {
        Err(ToolError::Failed(format!(
            "`{}` is the main session's alone: an agent works with read, edit and bash, \
             and decides on its best assumption where it would ask",
            self.0.name
        )))
    }
}
