//! [`Rig`]: the built agent of a lane and what a task needs around it.

use std::sync::Mutex;
use std::sync::atomic::AtomicU64;

use agent::agent::Agent;
use agent::conversation::Conversation;
use contracts::history::Session;
use decision_loop::meter::Meter;
use session::Sessions;
use tools::workspace::Workspace;

use super::Shared;
use crate::agents::Delegation;
use crate::brake::Brake;
use crate::checkpoints::Checkpoints;
use crate::choice::Choice;
use crate::cli::RunOptions;
use crate::interrupt::Interrupt;
use crate::jobs::{self, Jobs};
use crate::live::Live;
use crate::mode::{self, Mode, ModeCell};
use crate::output::{Output, lock};
use crate::providers::Provider;
use crate::tui::Ui;
use crate::tui::event::Event;

/// The built agent and what a task needs around it; one per process, for every task it runs.
pub struct Rig<'a> {
    pub agent: Agent<'a>,
    /// What the lane shares with the others: the browser's page among them.
    pub shared: &'a Shared,
    pub options: &'a RunOptions,
    pub workspace: &'a Workspace,
    pub output: &'a Output,
    pub brake: &'a Brake<'a>,
    /// Whether the person stopped the task running.
    pub interrupt: &'a Interrupt,
    /// The decision model's spend, over all tasks.
    pub meter: &'a Meter,
    pub history: &'a Sessions,
    /// The terminal sessions `bash` keeps open: in a session they outlive its tasks; one task of
    /// `exec` closes them when it ends.
    pub sessions: &'a tools::shell::Sessions,
    /// The agents the session started: let go with it, and what they spend is the task's.
    pub agents: &'a dyn Delegation,
    /// The monitors and schedules running in the background.
    pub jobs: &'a Jobs,
    pub jobs_kit: &'a jobs::Kit<'a>,
    /// The model's context window, for the agents (set at each task).
    pub window: &'a AtomicU64,
    /// The files as each task of the conversation found them, for `/rewind`.
    pub checkpoints: &'a Checkpoints,
    /// What the session's tasks spent, and when it began: `/usage`.
    pub spent: agent::agent::Spend,
    pub begun: std::time::Instant,
    pub session: Session,
    /// The conversation so far: the next task goes on from it.
    pub conversation: Option<Conversation>,
    /// The mode now: the gate and the person read it at each question.
    pub mode: &'a ModeCell,
    pub terminal: bool,
    pub asks: bool,
    /// The mode the model last heard of.
    pub told: Mode,
    /// The mode plan mode was entered from, which carrying out the plan goes back to.
    pub before_plan: Mode,
    /// The interface the session is shown in, at a terminal.
    pub ui: Option<Ui>,
    /// The pictures the person attached to the next task.
    pub attached: Vec<std::path::PathBuf>,
    /// What the agent thinks with now.
    pub choice: Choice,
    pub live: Option<&'a Live>,
    /// What commands never see, read at each command.
    pub hidden: &'a Mutex<Vec<String>>,
    /// What is known of the session beside its tasks, made in the background, not saved yet.
    pub meta: crate::meta::Pending,
}

impl Rig<'_> {
    /// Keep the variables of `provider`'s key from commands too, from the next command on: a
    /// provider chosen after the session began. Those of the one before stay hidden.
    pub fn hide_for(&self, provider: &Provider) {
        let mut hidden = lock(self.hidden);
        for name in provider.hidden() {
            if !hidden.contains(&name) {
                hidden.push(name);
            }
        }
    }

    /// Work in `mode` from the next question on; entering plan remembers the mode it came from.
    pub fn switch(&mut self, mode: Mode) {
        let now = self.mode.get();
        if mode == Mode::Plan && now != Mode::Plan {
            self.before_plan = now;
        }
        self.mode.set(mode);
        if let Some(ui) = &self.ui {
            ui.send(Event::Mode(mode.name().to_string()));
        }
    }

    /// The conversation the next task goes on from, told first when the mode changed since the
    /// model last heard of it (in the context, or in an earlier note). Nothing is said when it
    /// did not change, so the conversation's prefix stays the same for the prompt cache.
    pub fn told_of_mode(&mut self) -> Option<Conversation> {
        let conversation = self.conversation.take();
        let mode = self.mode.get();
        if mode == self.told {
            return conversation;
        }
        self.told = mode;
        let note = mode::notice(mode, self.terminal, self.asks);
        Some(self.agent.tell(conversation, &note))
    }
}
