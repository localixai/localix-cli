//! The composition root: the one place that builds the packages and hands them to the agent
//! through its ports. [`compose`] builds the providers (the chat model, and the decision model
//! behind the browser's hands); [`assemble`] builds the rest around any models: the brake, the
//! hands, the workspace, the mode's gate, the person, and the tools: `read`, `edit`, `bash`,
//! `browser` where a window can be shown, and `ask` in a session. The window itself opens only
//! when the browser is first used. The terminal sessions `bash` keeps are made here too, so they
//! are closed when `assemble` returns, before `main` exits (which would skip their `Drop`).

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use agent::agent::Agent;
use agent::conversation::Conversation;
use contracts::chat::Chat;
use contracts::history::Session;
use contracts::model::Decider;
use contracts::person::{Person, Question, Reply};
use contracts::tool::Tool;
use decision_loop::decision_loop::DecisionLoop;
use decision_loop::meter::{Meter, MeteredDecider};
use decision_loop::step::GoalStep;
use provider_kit::App;
use serde_json::Value;
use session::Sessions;
use tools::ask::AskTool;
use tools::browser::{Browser, Page};
use tools::workspace::Workspace;

use crate::agents::inbox::MainInbox;
use crate::agents::kit::{Kit, Own, Tools};
use crate::agents::{AgentsTool, Delegation, Registry};
use crate::brake::{Brake, BrakedDecider};
use crate::checkpoints::Checkpoints;
use crate::choice::Choice;
use crate::cli::RunOptions;
use crate::gate::{Stoppable, Tty};
use crate::instructions;
use crate::interrupt::{self, Interrupt};
use crate::jobs::{self, Jobs, JobsTool};
use crate::live::Live;
use crate::mode::{self, Mode, ModeCell, ModeGate};
use crate::output::{Output, lock};
use crate::providers::Provider;
use crate::tui::Ui;
use crate::tui::asking::{UiGate, UiPerson};
use crate::tui::event::Event;
use crate::windows;

/// Who calls the providers: attribution, the sign-in pages, the folder sign-ins are saved in.
pub const APP: App = App {
    name: "llx",
    slug: "llx",
    version: env!("CARGO_PKG_VERSION"),
};

/// Everything a session is built from, decided before the browser's event loop takes the main
/// thread (so a bad argument still fails fast, with no window machinery started).
pub struct Setup {
    pub options: RunOptions,
    /// The provider, model and reasoning level the agent thinks with.
    pub choice: Choice,
    pub workspace: Workspace,
    /// The agent's context block: the environment, the instructions, an earlier session.
    pub context: String,
    /// The same with no earlier session: what `/new` starts over with.
    pub fresh: String,
    pub schema: Option<Value>,
    /// The mode the run starts in, as the context states it.
    pub mode: Mode,
    /// Whether there is a terminal to ask the person on.
    pub terminal: bool,
    /// Whether `ask` is offered: in a session, not in `exec`, where one task runs unattended.
    pub asks: bool,
    pub history: Sessions,
    /// The session the tasks are recorded in: new, or resumed.
    pub session: Session,
    /// The interface a session at a terminal is shown in.
    pub ui: Option<Ui>,
    /// Whether the browser and terminal windows show by themselves (`windows::shown`).
    pub shown: bool,
    /// The person's terminal colours, which terminal windows take.
    #[cfg(unix)]
    pub theme: Option<tools::shell::Theme>,
}

/// What every lane of a session shares: the options, the workspace, the browser's page (one
/// action at a time), the session store, and the fresh context, the same bytes for each lane so
/// the prompt cache is shared too.
pub struct Shared {
    pub options: RunOptions,
    pub workspace: Workspace,
    pub page: Option<Page>,
    /// The browser the terminal windows open in, beside the page.
    pub browser: Option<Browser>,
    /// Whether the windows show by themselves (`windows::shown`).
    pub shown: bool,
    /// The person's terminal colours, which terminal windows take.
    #[cfg(unix)]
    pub theme: Option<tools::shell::Theme>,
    /// One browser action at a time, every lane's and every agent's.
    pub browser_lock: Mutex<()>,
    pub history: Sessions,
    /// The context with no earlier session: what a new lane starts with.
    pub fresh: String,
    /// The mode, models and terminal a new lane starts with.
    pub started: Mode,
    pub choice: Choice,
    pub terminal: bool,
    pub asks: bool,
    /// How many lanes are in a task now.
    pub working: std::sync::atomic::AtomicUsize,
    /// Every lane's queue, for what the window says.
    pub lanes: Arc<crate::arrivals::Lanes>,
    /// Terminal sessions' ids, shared by the lanes: an id names one session in the one window.
    pub terminal_ids: Arc<std::sync::atomic::AtomicU32>,
}

/// What one lane is built from beside what it shares.
pub struct Seat {
    pub context: String,
    pub session: Session,
    pub ui: Option<Ui>,
    pub choice: Choice,
    pub mode: Mode,
    pub schema: Option<Value>,
}

impl Setup {
    /// What the lanes share, with the page on `browser`, and the first lane's seat.
    pub fn split(self, browser: Option<Browser>) -> (Shared, Seat) {
        let shown = self.shown;
        let lanes = Arc::new(crate::arrivals::Lanes::default());
        // What the person does in the window reaches the lanes' runs.
        if let Some(browser) = &browser {
            let told = lanes.clone();
            browser.listen(move |tab, what| told.post(tab, what));
        }
        let shared = Shared {
            page: browser.clone().map(|browser| Page::new(browser, !shown)),
            browser,
            shown,
            #[cfg(unix)]
            theme: self.theme,
            browser_lock: Mutex::new(()),
            options: self.options,
            workspace: self.workspace,
            history: self.history,
            fresh: self.fresh,
            started: self.mode,
            choice: self.choice.clone(),
            terminal: self.terminal,
            asks: self.asks,
            working: Default::default(),
            lanes,
            terminal_ids: Arc::default(),
        };
        let seat = Seat {
            context: self.context,
            session: self.session,
            ui: self.ui,
            choice: self.choice,
            mode: self.mode,
            schema: self.schema,
        };
        (shared, seat)
    }
}

impl Shared {
    /// The seat of a new lane, recorded in `session`: the fresh context, as the session began.
    pub fn seat(&self, mut session: Session, ui: Option<Ui>) -> Seat {
        if session.workspace.is_empty() {
            session.workspace = self.workspace.root().display().to_string();
        }
        Seat {
            context: self.fresh.clone(),
            session,
            ui,
            choice: self.choice.clone(),
            mode: self.started,
            schema: None,
        }
    }
}

/// The two models a session thinks with.
pub struct Models<'a> {
    pub chat: &'a dyn Chat,
    /// Behind the browser's hands.
    pub decider: &'a dyn Decider,
    /// Where `/login` puts the models it chose; `None` when they cannot change.
    pub live: Option<&'a Live>,
}

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

/// Build the agent from `setup` on the chosen provider's chat model and OpenRouter's decision
/// model, with the browser when `browser` is given, and run `work` with it; its exit code.
pub fn compose(setup: Setup, browser: Option<Browser>, work: impl FnOnce(&mut Rig) -> i32) -> i32 {
    let (shared, seat) = setup.split(browser);
    lane(&shared, seat, work)
}

/// One lane over `shared`: its models, its stop (found by its session id), its events tagged
/// with that id (the desk told the interface of it); `work` runs with it.
pub fn lane(shared: &Shared, seat: Seat, work: impl FnOnce(&mut Rig) -> i32) -> i32 {
    let live = Live::of(&seat.choice, &shared.options.models.decision_model);
    let models = Models {
        chat: &live.chat,
        decider: &live.decider,
        live: Some(&live),
    };
    lane_with(shared, seat, models, work)
}

/// [`lane`] around `models`.
pub fn lane_with(
    shared: &Shared,
    mut seat: Seat,
    models: Models,
    work: impl FnOnce(&mut Rig) -> i32,
) -> i32 {
    let interrupt = Arc::new(Interrupt::new());
    let lane = Arc::clone(&interrupt);
    let _known = interrupt::register(&seat.session.id, move || lane.stop());
    seat.ui = seat.ui.map(|ui| ui.for_lane(&seat.session.id));
    let output = Output::new(shared.options.json, shared.options.verbose).in_ui(seat.ui.clone());
    assemble(shared, seat, models, &output, &interrupt, work)
}

/// Build a lane's agent from `seat` over `shared`, around `models`, reporting on `output` and
/// stopped by `interrupt`, and run `work` with it.
pub fn assemble(
    shared: &Shared,
    seat: Seat,
    models: Models,
    output: &Output,
    interrupt: &Interrupt,
    work: impl FnOnce(&mut Rig) -> i32,
) -> i32 {
    let Shared {
        options,
        workspace,
        page,
        browser_lock,
        history,
        fresh,
        terminal,
        asks,
        ..
    } = shared;
    let (terminal, asks) = (*terminal, *asks);
    let Seat {
        context,
        session,
        ui,
        choice,
        mode: started,
        schema,
    } = seat;
    let closed = || output.closed();
    let stopped = || interrupt.stopped();
    let brake = Brake::new(models.chat, &closed, &stopped);
    let meter = Meter::default();
    let braked = BrakedDecider {
        inner: models.decider,
        interrupt,
    };
    let decider = MeteredDecider {
        inner: &braked,
        meter: &meter,
    };
    let on_step = |step: &GoalStep| output.step(step);
    let hands = DecisionLoop {
        decider: &decider,
        on_step: &on_step,
        max_actions: options.max_steps,
    };
    let mode = ModeCell::new(started);
    let waited = |took| brake.extend(took);
    let approver = match &ui {
        Some(ui) => {
            let stopped = &stopped;
            let asker = Box::new(UiGate {
                ui: ui.clone(),
                stopped,
            });
            ModeGate::asking(&mode, Some(asker), &waited)
        }
        None => ModeGate::new(&mode, terminal.then_some(Tty), &waited),
    };
    let gate = Stoppable {
        inner: &approver,
        stopped: &stopped,
    };
    let person: Box<dyn Person> = match &ui {
        Some(ui) => Box::new(UiPerson {
            ui: ui.clone(),
            mode: &mode,
            stopped: &stopped,
            waited: &waited,
        }),
        None => Box::new(Nobody),
    };
    let ask = AskTool {
        person: person.as_ref(),
    };
    // The one window as its tabs, where it shows.
    let tabs = (shared.browser.clone())
        .filter(|_| shared.shown)
        .map(crate::windows::Tabs);
    // What commands never see: the variables of the key the models are called with, and of a
    // provider chosen later with `/login` (hiding is no barrier anyway).
    let hidden_names = Mutex::new(choice.provider.hidden());
    let hidden = || lock(&hidden_names).clone();
    // The lane's queue: what reaches its run from outside its calls.
    let lane_queue = Arc::new(crate::arrivals::queue(output.ui().cloned()));
    shared.lanes.join(&lane_queue);
    // Terminal windows open only where windows show; a call can still show or hide one.
    #[cfg(unix)]
    let sessions = windows::terminals(
        (shared.browser.clone()).map(|b| {
            let owner = (shared.lanes.clone(), Arc::downgrade(&lane_queue));
            windows::panels(b, shared.theme, owner)
        }),
        shared.shown,
    )
    .numbered(shared.terminal_ids.clone());
    #[cfg(not(unix))]
    let sessions = tools::shell::Sessions::default();
    let checkpoints = Checkpoints::default();
    let window = AtomicU64::new(choice.selection.model().context);
    // Each participant keeps its own conversation; one pipeline prepares them all.
    let strategy = agent::history::Pipeline::default();
    // Agents build tools of their own from these, on a gate that names them, with neither
    // `agents` (one level) nor `ask` (nobody answers an agent).
    let kit = Kit {
        chat: &brake,
        workspace,
        gate: &gate,
        hidden: &hidden,
        stopped: &stopped,
        sessions: &sessions,
        browser: page.as_ref(),
        hands: &hands,
        windowed: tabs
            .as_ref()
            .map(|tabs| tabs as &dyn tools::window::Windowed),
        browser_lock,
        context: fresh,
        max_turns: options.max_turns,
        window: &window,
        strategy: &strategy,
        checkpoints: &checkpoints,
        output,
        history,
        mode: &mode,
        started,
        terminal,
    };
    // The main session's tools are an agent's, on the session's own gate and stop. A
    // subdirectory's own instructions come with the first call that touches a file there; the
    // directories the context already covers are known.
    let own = Own {
        gate: &gate,
        stop: &stopped,
        seen: Mutex::new(instructions::chain(workspace.root()).into_iter().collect()),
    };
    let main_tools = Tools::new(&kit, &own);
    let registry = Registry::new(lane_queue.clone());
    let jobs = Jobs::of(&session.id);
    let jobs_kit = jobs::Kit {
        workspace,
        gate: &gate,
        hidden: &hidden,
        output,
        session: asks,
        queue: &lane_queue,
    };
    // The wait between turns ends at the time limit too; the gates and tools keep `stopped`.
    let waiting = || stopped() || brake.due();
    let inbox = MainInbox {
        registry: &registry,
        queue: &lane_queue,
        jobs: &jobs,
        jobs_kit: &jobs_kit,
        output,
        stopped: &waiting,
    };
    // The agents' threads end with the session at the latest: each task waits for its own. The
    // guard ends the jobs and stops the agents even when the session unwinds from a panic, or
    // the scope would wait for their threads for ever.
    std::thread::scope(|scope| {
        // Inside the scope: it is dropped while unwinding, before the scope waits for threads.
        let _ending = Ending {
            jobs: &jobs,
            registry: &registry,
        };
        let agents = AgentsTool::new(&kit, &registry, scope);
        agents.adopt(&session.id);
        let jobs_tool = JobsTool::new(&jobs_kit, &jobs, scope);
        if asks {
            scope.spawn(|| jobs.tick(&jobs_kit));
        }
        main_tools.with(|own| {
            let mut tools: Vec<&dyn Tool> = own.to_vec();
            if asks {
                tools.push(&ask);
            }
            tools.push(&agents);
            tools.push(&jobs_tool);
            let agent = Agent {
                chat: &brake,
                tools: &tools,
                context: &context,
                max_turns: options.max_turns,
                observer: output,
                schema: schema.as_ref(),
                inbox: Some(&inbox),
                window: 0,
                strategy: &strategy,
            };
            let mut rig = Rig {
                agent,
                shared,
                options,
                workspace,
                output,
                brake: &brake,
                interrupt,
                meter: &meter,
                history,
                sessions: &sessions,
                agents: &agents,
                jobs: &jobs,
                jobs_kit: &jobs_kit,
                window: &window,
                checkpoints: &checkpoints,
                spent: agent::agent::Spend::default(),
                begun: std::time::Instant::now(),
                session,
                conversation: None,
                mode: &mode,
                terminal,
                asks,
                told: started,
                // `llx --mode plan`: carrying out the plan goes on in the default mode.
                before_plan: match started {
                    Mode::Plan => Mode::Default,
                    other => other,
                },
                ui,
                attached: vec![],
                choice,
                live: models.live,
                hidden: &hidden_names,
                meta: crate::meta::Pending::default(),
            };
            let code = work(&mut rig);
            rig.agents.settle();
            code
        })
    })
}

/// Who answers with no interface (`exec`, where `ask` is not offered anyway): nobody.
struct Nobody;

impl Person for Nobody {
    fn ask(&self, _: &[Question]) -> Reply {
        Reply::Nobody("nobody is at a terminal to answer".into())
    }
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

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
