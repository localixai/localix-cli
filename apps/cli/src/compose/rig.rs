//! [`Rig`]: the built agent of a lane and what a task needs around it, built by [`drive`] from
//! what `assemble` and `equip` made, and the work run with it.

use std::sync::Mutex;
use std::sync::atomic::AtomicU64;

use agent::agent::Agent;
use agent::conversation::Conversation;
use contracts::history::Session;
use contracts::tool::Tool;
use decision_loop::meter::Meter;
use session::Sessions;
use tools::workspace::Workspace;

use super::{Base, Lane, Seat, Shared};
use crate::agents::{AgentsTool, Delegation, Registry};
use crate::brake::Brake;
use crate::checkpoints::Checkpoints;
use crate::choice::Choice;
use crate::cli::RunOptions;
use crate::interrupt::Interrupt;
use crate::jobs::{self, Jobs, JobsTool};
use crate::live::Live;
use crate::mode::{self, Mode, ModeCell};
use crate::modes::Modes;
use crate::output::{Output, lock};
use crate::providers::Provider;
use crate::rules::Rules;
use crate::server::Ui;
use protocol::Event;

/// The built agent and what a task needs around it; one per process, for every task it runs.
pub(crate) struct Rig<'a> {
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
    pub spent: contracts::chat::Spend,
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
    /// Every mode the session can switch to, and the command line's rules beside each.
    pub modes: &'a Modes,
    pub rules: &'a Rules,
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

/// The agent built from `base` and `lane` with `seat`'s session, and `work` run with it. The
/// agents' threads end with the session at the latest: each task waits for its own. The guard
/// ends the jobs and stops the agents even when the session unwinds from a panic, or the scope
/// would wait for their threads for ever.
pub(super) fn drive(
    base: &Base,
    lane: &Lane,
    seat: Seat,
    work: impl FnOnce(&mut Rig) -> i32,
) -> i32 {
    let Seat {
        context,
        session,
        ui,
        choice,
        mode: started,
        schema,
    } = seat;
    let shared = base.shared;
    let options = &shared.options;
    std::thread::scope(|scope| {
        // Inside the scope: it is dropped while unwinding, before the scope waits for threads.
        let _ending = Ending {
            jobs: lane.jobs,
            registry: lane.registry,
        };
        let agents = AgentsTool::new(lane.kit, lane.registry, scope);
        agents.adopt(&session.id);
        let jobs_tool = JobsTool::new(lane.jobs_kit, lane.jobs, scope);
        if shared.asks {
            let thread = std::thread::Builder::new().name("llx-scheduler".into());
            if let Err(why) = thread.spawn_scoped(scope, || lane.jobs.tick(lane.jobs_kit)) {
                let said =
                    format!("llx: schedules will not run in this session (no thread: {why})");
                base.output.note(&said);
            }
        }
        lane.tools.with(|own| {
            let mut tools: Vec<&dyn Tool> = own.to_vec();
            if shared.asks {
                tools.push(base.ask);
            }
            tools.push(&agents);
            tools.push(&jobs_tool);
            let agent = Agent {
                chat: base.brake,
                tools: &tools,
                context: &context,
                max_turns: options.max_turns,
                observer: base.output,
                schema: schema.as_ref(),
                inbox: Some(lane.inbox),
                window: 0,
                strategy: lane.strategy,
            };
            let mut rig = Rig {
                agent,
                shared,
                options,
                workspace: &shared.workspace,
                output: base.output,
                brake: base.brake,
                interrupt: base.interrupt,
                meter: base.meter,
                history: &shared.history,
                sessions: lane.sessions,
                agents: &agents,
                jobs: lane.jobs,
                jobs_kit: lane.jobs_kit,
                window: lane.window,
                checkpoints: lane.checkpoints,
                spent: contracts::chat::Spend::default(),
                begun: std::time::Instant::now(),
                session,
                conversation: None,
                mode: base.mode,
                terminal: shared.terminal,
                asks: shared.asks,
                told: started,
                modes: &shared.modes,
                rules: &shared.rules,
                ui,
                attached: vec![],
                choice,
                live: base.live,
                hidden: lane.hidden,
                meta: crate::meta::Pending::default(),
            };
            let code = work(&mut rig);
            rig.agents.settle();
            code
        })
    })
}

/// Ends the session's background work when it is dropped.
struct Ending<'a> {
    jobs: &'a Jobs,
    registry: &'a Registry,
}

impl Drop for Ending<'_> {
    fn drop(&mut self) {
        self.jobs.close();
        self.registry.stop_all();
    }
}

impl Rig<'_> {
    /// Keep the variables of `provider`'s key from commands too, from the next command on: a
    /// provider chosen after the session began. Those of the one before stay hidden.
    pub(crate) fn hide_for(&self, provider: &Provider) {
        let mut hidden = lock(self.hidden);
        for name in provider.hidden() {
            if !hidden.contains(&name) {
                hidden.push(name);
            }
        }
    }

    /// Work in `mode` from the next question on.
    pub(crate) fn switch(&mut self, mode: Mode) {
        self.mode.set(mode);
        if let Some(ui) = &self.ui {
            ui.send(Event::Mode(self.mode.get().name().to_string()));
        }
    }

    /// The conversation the next task goes on from, told first when the mode changed since the
    /// model last heard of it (in the context, or in an earlier note). Nothing is said when it
    /// did not change, so the conversation's prefix stays the same for the prompt cache.
    pub(crate) fn told_of_mode(&mut self) -> Option<Conversation> {
        let conversation = self.conversation.take();
        let mode = self.mode.get();
        if mode == self.told {
            return conversation;
        }
        self.told = mode.clone();
        let note = mode::notice(&mode, self.rules, self.terminal, self.asks);
        Some(self.agent.tell(conversation, &note))
    }
}
