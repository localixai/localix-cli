//! The composition root: the one place that builds the packages and hands them to the agent
//! through its ports. [`compose`] builds the providers (the chat model, and the decision model
//! behind the browser's hands); [`assemble`] builds the rest around any models: the brake, the
//! hands, the workspace, the mode's gate, the person, and the tools: `read`, `edit`, `bash`,
//! `browser` where a window can be shown, and `ask` in a session. The window itself opens only
//! when the browser is first used. The terminal sessions `bash` keeps are made here too, so they
//! are closed when `assemble` returns, before `main` exits (which would skip their `Drop`).

use std::cell::{Cell, RefCell};

use agent::agent::Agent;
use agent::conversation::Conversation;
use contracts::chat::Chat;
use contracts::gate::Gate;
use contracts::history::{History, Session};
use contracts::model::Decider;
use contracts::person::Person;
use contracts::tool::Tool;
use decision_loop::decision_loop::DecisionLoop;
use decision_loop::meter::{Meter, MeteredDecider};
use decision_loop::step::GoalStep;
use openrouter::auth::KEY_VARIABLE;
use provider_kit::App;
use serde_json::Value;
use session::Sessions;
use tools::ask::AskTool;
use tools::browser::{Browser, BrowserTool, Page};
use tools::fs::{EditTool, ReadTool};
use tools::shell::BashTool;
use tools::workspace::Workspace;

use crate::brake::{Brake, BrakedDecider};
use crate::choice::Choice;
use crate::cli::RunOptions;
use crate::gate::{Stoppable, Tty};
use crate::instructions::{self, Instructed};
use crate::interrupt::{self, Interrupt};
use crate::live::Live;
use crate::mode::{self, Mode, ModeGate};
use crate::output::Output;
use crate::person::TerminalPerson;
use crate::tui::Ui;
use crate::tui::asking::{UiGate, UiPerson};

/// Who calls the providers: attribution, the sign-in pages, the folder sign-ins are saved in.
pub const APP: App = App {
    name: "llx",
    slug: "llx",
    version: env!("CARGO_PKG_VERSION"),
};

/// What commands never see: the key the models are called with.
const HIDDEN: &[&str] = &[KEY_VARIABLE];

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
    /// Whether `ask` is offered: in a session, not in `exec`, as Codex leaves it out of
    /// `codex exec`: one task runs unattended.
    pub asks: bool,
    pub history: Sessions,
    /// The session the tasks are recorded in: new, or resumed.
    pub session: Session,
    /// The interface a session at a terminal is shown in.
    pub ui: Option<Ui>,
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
    pub options: &'a RunOptions,
    pub workspace: &'a Workspace,
    pub output: &'a Output,
    pub brake: &'a Brake<'a>,
    /// Whether the person stopped the task running.
    pub interrupt: &'a Interrupt,
    /// The decision model's spend, over all tasks.
    pub meter: &'a Meter,
    /// The browser's page, where there is a browser.
    pub page: Option<&'a Page>,
    pub history: &'a Sessions,
    /// The terminal sessions `bash` keeps open: closed when each task ends.
    pub sessions: &'a tools::shell::Sessions,
    pub session: Session,
    /// The conversation so far: the next task goes on from it.
    pub conversation: Option<Conversation>,
    /// The context with no earlier session, for `/new`.
    pub fresh: &'a str,
    /// The mode now: the gate and the person read it at each question.
    pub mode: &'a Cell<Mode>,
    pub terminal: bool,
    pub asks: bool,
    /// The mode the context states.
    pub started: Mode,
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
}

impl Rig<'_> {
    /// `/new`: a new conversation in a new session, with nothing of the earlier one.
    pub fn start_over(&mut self) {
        self.sessions.close_all();
        self.conversation = None;
        self.agent.context = self.fresh;
        self.told = self.started;
        self.session = self.history.start();
        self.session.workspace = self.workspace.root().display().to_string();
    }

    /// Work in `mode` from the next question on; entering plan remembers the mode it came from.
    pub fn switch(&mut self, mode: Mode) {
        let now = self.mode.get();
        if mode == Mode::Plan && now != Mode::Plan {
            self.before_plan = now;
        }
        self.mode.set(mode);
        if let Some(ui) = &self.ui {
            ui.send(crate::tui::event::Event::Mode(mode.name().to_string()));
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
    let live = Live::of(&setup.choice, &setup.options.models.decision_model);
    let models = Models {
        chat: &live.chat,
        decider: &live.decider,
        live: Some(&live),
    };
    let output = Output::new(setup.options.json, setup.options.verbose).in_ui(setup.ui.clone());
    assemble(setup, models, &output, &interrupt::PROCESS, browser, work)
}

/// Build the agent from `setup` around `models`, reporting on `output` and stopped by
/// `interrupt`, and run `work` with it.
pub fn assemble(
    setup: Setup,
    models: Models,
    output: &Output,
    interrupt: &Interrupt,
    browser: Option<Browser>,
    work: impl FnOnce(&mut Rig) -> i32,
) -> i32 {
    let Setup {
        options,
        workspace,
        context,
        fresh,
        schema,
        mode: started,
        terminal,
        asks,
        history,
        session,
        ui,
        choice,
        ..
    } = setup;
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
    let mode = Cell::new(started);
    let waited = |took| brake.extend(took);
    let approver = match &ui {
        Some(ui) => {
            let gate = || Some(Box::new(UiGate::new(ui.clone(), &stopped)) as Box<dyn Gate>);
            ModeGate::asking(&mode, gate(), gate(), &waited)
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
        None => Box::new(TerminalPerson {
            prompter: Tty,
            mode: &mode,
            stopped: &stopped,
            waited: &waited,
        }),
    };
    let ask = AskTool {
        person: person.as_ref(),
    };
    // Shown in a session, so the person watches what the agent does there; hidden when nobody
    // watches (exec) or nobody is to be asked (autonomous). A call can still choose either way.
    let shown = options.show_browser || (asks && started != Mode::Autonomous);
    let page = browser.map(|browser| Page::new(browser, !shown));
    let browser_tool = page.as_ref().map(|page| BrowserTool {
        page,
        hands: &hands,
        gate: &gate,
    });
    let read = ReadTool {
        workspace: &workspace,
    };
    let edit = EditTool {
        workspace: &workspace,
        gate: &gate,
    };
    let sessions = tools::shell::Sessions::default();
    let bash = BashTool {
        workspace: &workspace,
        gate: &gate,
        hidden: HIDDEN,
        stop: &stopped,
        sessions: &sessions,
    };
    // A subdirectory's own instructions come with the first call that touches a file there; the
    // directories the context already covers are known.
    let root = workspace.root().to_path_buf();
    let seen = RefCell::new(instructions::chain(&root).into_iter().collect());
    let read = Instructed {
        inner: &read,
        root: root.clone(),
        seen: &seen,
    };
    let edit = Instructed {
        inner: &edit,
        root,
        seen: &seen,
    };
    let mut tools: Vec<&dyn Tool> = vec![&read, &edit, &bash];
    if let Some(browser_tool) = &browser_tool {
        tools.push(browser_tool);
    }
    if asks {
        tools.push(&ask);
    }
    let agent = Agent {
        chat: &brake,
        tools: &tools,
        context: &context,
        max_turns: options.max_turns,
        observer: output,
        schema: schema.as_ref(),
    };
    let mut rig = Rig {
        agent,
        options: &options,
        workspace: &workspace,
        output,
        brake: &brake,
        interrupt,
        meter: &meter,
        page: page.as_ref(),
        history: &history,
        sessions: &sessions,
        session,
        conversation: None,
        fresh: &fresh,
        mode: &mode,
        terminal,
        asks,
        started,
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
    };
    work(&mut rig)
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
