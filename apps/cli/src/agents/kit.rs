//! What an agent's tools are built from, and the tools built: the main session's, and each
//! agent's own, on a gate that names it and a stop of its own (the task's, or the person's `x` on
//! its row); each learns a subtree's instructions on its own. The browser is one window for
//! everyone: one action at a time, whoever asks.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use contracts::chat::Chat;
use contracts::gate::Gate;
use contracts::hands::Hands;
use contracts::history::History;
use contracts::tool::Tool;
use tools::browser::Page;
use tools::fs::{EditTool, ReadTool};
use tools::shell::BashTool;
use tools::window::Windowed;
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
    pub sessions: &'a tools::shell::Sessions,
    pub browser: Option<&'a Page>,
    /// Who carries out a goal on any tab of the window.
    pub hands: &'a dyn Hands,
    /// The window the person watches, where it shows.
    pub windowed: Option<&'a dyn Windowed>,
    /// One browser action at a time, the main session's and every agent's.
    pub browser_lock: &'a crate::kinds::BrowserLocks,
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
    pub history: &'a (dyn History + Sync),
    pub mode: &'a ModeCell,
    /// The mode the context states.
    pub started: Mode,
    /// The command line's rules, beside each mode's.
    pub rules: crate::rules::Rules,
    pub terminal: bool,
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

/// A session's tools: `read`, `edit`, `bash` (commands run to their end), and `window`, whose
/// tabs are terminal sessions and the browser's pages where there is one.
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
                sessions: kit.sessions,
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
        let parts = crate::kinds::Parts {
            bash: &self.bash,
            sessions: kit.sessions,
            stop: own.stop,
            page: kit.browser,
            locks: kit.browser_lock,
            windowed: kit.windowed,
            hands: kit.hands,
            gate: own.gate,
        };
        crate::kinds::window(&parts, |window| {
            let mut tools: Vec<&dyn Tool> = vec![&read, &edit, &self.bash];
            if let Some(window) = window {
                tools.push(window);
            }
            work(&tools)
        })
    }
}
