//! The composition root: the one place that builds the packages and hands them to the agent
//! through its ports. [`compose`] builds the providers (the chat model, and the decision model
//! behind the browser's hands); [`assemble`] builds the rest around any models: the brake, the
//! hands, the workspace, the mode's gate, the person, and the tools: `read`, `edit`, `bash`,
//! `browser` where a window can be shown, and `ask` in a session. The window itself opens only
//! when the browser is first used. The terminal sessions `bash` keeps are made here too, so they
//! are closed when `assemble` returns, before `main` exits (which would skip their `Drop`).

use std::cell::Cell;

use agent::agent::Agent;
use agent::conversation::Conversation;
use contracts::chat::Chat;
use contracts::history::{History, Session};
use contracts::model::Decider;
use contracts::tool::Tool;
use decision_loop::decision_loop::DecisionLoop;
use decision_loop::meter::{Meter, MeteredDecider};
use decision_loop::step::GoalStep;
use openrouter::App;
use openrouter::auth::KEY_VARIABLE;
use openrouter::chat::ChatModel;
use openrouter::client::OpenRouter;
use openrouter::system_one::SystemOne;
use serde_json::Value;
use session::Sessions;
use tools::ask::AskTool;
use tools::browser::{Browser, BrowserTool, Page};
use tools::fs::{EditTool, ReadTool};
use tools::shell::BashTool;
use tools::workspace::Workspace;

use crate::brake::{Brake, BrakedDecider};
use crate::cli::RunOptions;
use crate::gate::{Stoppable, Tty};
use crate::interrupt::{self, Interrupt};
use crate::mode::{self, Mode, ModeGate};
use crate::output::Output;
use crate::person::TerminalPerson;

/// Who calls OpenRouter: its attribution, the sign-in page, the folder the key is saved in.
pub const APP: App = App {
    name: "Localix CLI",
    slug: "lx",
    version: env!("CARGO_PKG_VERSION"),
};

/// What commands never see: the key the models are called with.
const HIDDEN: &[&str] = &[KEY_VARIABLE];

/// Everything a session is built from, decided before the browser's event loop takes the main
/// thread (so a bad argument still fails fast, with no window machinery started).
pub struct Setup {
    pub options: RunOptions,
    pub key: String,
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
}

/// The two models a session thinks with.
pub struct Models<'a> {
    pub chat: &'a dyn Chat,
    /// Behind the browser's hands.
    pub decider: &'a dyn Decider,
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

/// Build the agent from `setup` on OpenRouter's models, with the browser when `browser` is given,
/// and run `work` with it; its exit code is returned.
pub fn compose(setup: Setup, browser: Option<Browser>, work: impl FnOnce(&mut Rig) -> i32) -> i32 {
    let client = OpenRouter::new(setup.key.clone(), APP);
    let chat = ChatModel {
        client: &client,
        model: setup.options.models.model.clone(),
    };
    let system_one = SystemOne {
        client: &client,
        model: setup.options.models.decision_model.clone(),
    };
    let models = Models {
        chat: &chat,
        decider: &system_one,
    };
    let output = Output::new(setup.options.json, setup.options.verbose);
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
    let approver = ModeGate::new(&mode, terminal.then_some(Tty), &waited);
    let gate = Stoppable {
        inner: &approver,
        stopped: &stopped,
    };
    let person = TerminalPerson {
        prompter: Tty,
        mode: &mode,
        stopped: &stopped,
        waited: &waited,
    };
    let ask = AskTool { person: &person };
    let page = browser.map(|browser| Page::new(browser, !options.show_browser));
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
        // `lx --mode plan`: carrying out the plan goes on in the default mode.
        before_plan: match started {
            Mode::Plan => Mode::Default,
            other => other,
        },
    };
    work(&mut rig)
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
