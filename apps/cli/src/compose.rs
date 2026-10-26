//! The composition root: the one place that builds the packages and hands them to the agent
//! through its ports. [`compose`] builds the providers (the chat model, and the decision model
//! behind the browser's hands); [`assemble`] builds the rest around any models: the brake, the
//! hands, the workspace, the mode's gate, the person, and the tools: `read`, `edit`, `bash`,
//! `browser` where a window can be shown, and `ask` in a session. The window itself opens only
//! when the browser is first used. The terminal sessions `bash` keeps are made here too, so they
//! are closed when `assemble` returns, before `main` exits (which would skip their `Drop`).

mod lane;
mod rig;
mod setup;

#[cfg(test)]
pub(crate) use lane::lane_with;
pub(crate) use lane::{Models, compose, lane};
pub(crate) use rig::Rig;
pub(crate) use setup::{Seat, Setup, Shared};

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use contracts::gate::Gate;
use contracts::hands::Hands;
use contracts::person::{Person, Question, Reply};
use decision_loop::decision_loop::DecisionLoop;
use decision_loop::meter::{Meter, MeteredDecider};
use decision_loop::step::GoalStep;
use provider_kit::App;
use tools::ask::AskTool;

use crate::agents::Registry;
use crate::agents::inbox::MainInbox;
use crate::agents::kit::{Kit, Own, Tools};
use crate::brake::{Brake, BrakedDecider};
use crate::checkpoints::Checkpoints;
use crate::gate::{Stoppable, Tty};
use crate::instructions;
use crate::interrupt::Interrupt;
use crate::jobs::{self, Jobs};
use crate::live::Live;
use crate::mode::{ModeCell, ModeGate};
use crate::output::{Output, lock};
use crate::server::asking::{UiGate, UiPerson};
use crate::windows;

/// Who calls the providers: attribution, the sign-in pages, the folder sign-ins are saved in.
pub(crate) const APP: App = App {
    name: "llx",
    slug: "llx",
    version: env!("CARGO_PKG_VERSION"),
};

/// Build a lane's agent from `seat` over `shared`, around `models`, reporting on `output` and
/// stopped by `interrupt`, and run `work` with it: here the brake, the hands, the mode's gate and
/// the person; [`equip`] the rest.
pub(crate) fn assemble(
    shared: &Shared,
    seat: Seat,
    models: Models,
    output: &Output,
    interrupt: &Interrupt,
    work: impl FnOnce(&mut Rig) -> i32,
) -> i32 {
    let closed = || output.closed();
    let stopped = || interrupt.stopped();
    let brake = Brake::new(models.chat, &closed, &stopped);
    let meter = Meter::default();
    let braked = BrakedDecider {
        inner: models.decider,
        interrupt,
    };
    let traced = crate::hands::Traced::new(&braked);
    let decider = MeteredDecider {
        inner: &traced,
        meter: &meter,
    };
    let on_step = |step: &GoalStep| output.step(step);
    let hands = DecisionLoop {
        decider: &decider,
        on_step: &on_step,
        pause: &crate::hands::pause,
        max_actions: shared.options.max_steps,
    };
    let mode = ModeCell::new(seat.mode.clone());
    let waited = |took| brake.extend(took);
    let approver = match &seat.ui {
        Some(ui) => {
            let stopped = &stopped;
            let asker = Box::new(UiGate {
                ui: ui.clone(),
                stopped,
                asking: &shared.asking,
            });
            ModeGate::asking(&mode, Some(asker), &waited)
        }
        None => ModeGate::new(&mode, shared.terminal.then_some(Tty), &waited),
    };
    let approver = approver.ruled(shared.rules.clone());
    let gate = Stoppable {
        inner: &approver,
        stopped: &stopped,
    };
    let person: Box<dyn Person> = match &seat.ui {
        Some(ui) => Box::new(UiPerson {
            ui: ui.clone(),
            stopped: &stopped,
            waited: &waited,
            asking: &shared.asking,
        }),
        None => Box::new(Nobody),
    };
    let ask = AskTool {
        person: person.as_ref(),
    };
    let base = Base {
        shared,
        output,
        interrupt,
        brake: &brake,
        meter: &meter,
        hands: &hands,
        mode: &mode,
        gate: &gate,
        ask: &ask,
        stopped: &stopped,
        live: models.live,
    };
    equip(&base, seat, work)
}

/// The tools around `base`, the agents and the jobs, the lane's queue and its inbox; then the
/// rig is built and `work` run with it ([`rig::drive`]).
fn equip(base: &Base, seat: Seat, work: impl FnOnce(&mut Rig) -> i32) -> i32 {
    let (shared, output) = (base.shared, base.output);
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
    // The one window as its tabs, where it shows; the lane's are of its directory.
    let group = windows::group(&seat.session.workspace, workspace.root());
    // What commands never see: the variables of the key the models are called with, and of a
    // provider chosen later with `/login` (hiding is no barrier anyway).
    let hidden_names = Mutex::new(seat.choice.provider.hidden());
    let hidden = || lock(&hidden_names).clone();
    // The lane's queue: what reaches its run from outside its calls.
    let lane_queue = Arc::new(crate::arrivals::queue(output.ui().cloned()));
    shared.lanes.join(&lane_queue);
    let tabs = (shared.browser.clone())
        .filter(|_| shared.shown)
        .map(|browser| windows::Tabs::of(browser, &group).owned(&shared.lanes, &lane_queue));
    // Terminal windows open only where windows show; a call can still show or hide one.
    #[cfg(unix)]
    let sessions = windows::terminals(
        (shared.browser.clone()).map(|b| {
            let owner = (Arc::clone(&shared.lanes), Arc::downgrade(&lane_queue));
            let owner = (owner.0, owner.1, group.clone());
            windows::panels(b, shared.theme, owner)
        }),
        shared.shown,
    )
    .numbered(Arc::clone(&shared.terminal_ids));
    #[cfg(not(unix))]
    let sessions = tools::shell::Sessions::default();
    let checkpoints = Checkpoints::default();
    let window = AtomicU64::new(seat.choice.selection.model().context);
    // Each participant keeps its own conversation; one pipeline prepares them all.
    let strategy = agent::history::Pipeline::default();
    // Agents build tools of their own from these, on a gate that names them, with neither
    // `agents` (one level) nor `ask` (nobody answers an agent).
    let kit = Kit {
        chat: base.brake,
        workspace,
        gate: base.gate,
        hidden: &hidden,
        stopped: base.stopped,
        sessions: &sessions,
        browser: page.as_ref(),
        hands: base.hands,
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
        mode: base.mode,
        started: seat.mode.clone(),
        rules: shared.rules.clone(),
        terminal: *terminal,
    };
    // The main session's tools are an agent's, on the session's own gate and stop. A
    // subdirectory's own instructions come with the first call that touches a file there; the
    // directories the context already covers are known.
    let own = Own {
        gate: base.gate,
        stop: base.stopped,
        seen: Mutex::new(instructions::chain(workspace.root()).into_iter().collect()),
    };
    let main_tools = Tools::new(&kit, &own);
    let registry = Registry::new(Arc::clone(&lane_queue));
    let jobs = Jobs::of(&seat.session.id);
    let jobs_kit = jobs::Kit {
        workspace,
        gate: base.gate,
        hidden: &hidden,
        output,
        session: *asks,
        queue: &lane_queue,
    };
    // The wait between turns ends at the time limit too; the gates and tools keep `stopped`.
    let waiting = || (base.stopped)() || base.brake.due();
    let inbox = MainInbox {
        registry: &registry,
        queue: &lane_queue,
        jobs: &jobs,
        jobs_kit: &jobs_kit,
        output,
        stopped: &waiting,
    };
    let lane = Lane {
        kit: &kit,
        tools: &main_tools,
        registry: &registry,
        jobs: &jobs,
        jobs_kit: &jobs_kit,
        inbox: &inbox,
        sessions: &sessions,
        checkpoints: &checkpoints,
        window: &window,
        strategy: &strategy,
        hidden: &hidden_names,
    };
    rig::drive(base, &lane, seat, work)
}

/// What [`assemble`] made, for the rest of the lane to borrow.
struct Base<'a> {
    shared: &'a Shared,
    output: &'a Output,
    interrupt: &'a Interrupt,
    brake: &'a Brake<'a>,
    meter: &'a Meter,
    hands: &'a dyn Hands,
    mode: &'a ModeCell,
    gate: &'a dyn Gate,
    ask: &'a AskTool<'a>,
    stopped: &'a (dyn Fn() -> bool + Sync),
    live: Option<&'a Live>,
}

/// What [`equip`] made around the base: the tools, the agents, the jobs and the inbox.
struct Lane<'a> {
    kit: &'a Kit<'a>,
    tools: &'a Tools<'a>,
    registry: &'a Registry,
    jobs: &'a Jobs,
    jobs_kit: &'a jobs::Kit<'a>,
    inbox: &'a MainInbox<'a>,
    sessions: &'a tools::shell::Sessions,
    checkpoints: &'a Checkpoints,
    window: &'a AtomicU64,
    strategy: &'a agent::history::Pipeline,
    hidden: &'a Mutex<Vec<String>>,
}

/// Who answers with no interface (`exec`, where `ask` is not offered anyway): nobody.
struct Nobody;

impl Person for Nobody {
    fn ask(&self, _: &[Question]) -> Reply {
        Reply::Nobody("nobody is at a terminal to answer".into())
    }
}

#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
