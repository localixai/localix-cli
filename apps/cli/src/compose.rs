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
pub use lane::lane_with;
pub use lane::{Models, compose, lane};
pub use rig::Rig;
pub use setup::{Seat, Setup, Shared};

use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use agent::agent::Agent;
use contracts::person::{Person, Question, Reply};
use contracts::tool::Tool;
use decision_loop::decision_loop::DecisionLoop;
use decision_loop::meter::{Meter, MeteredDecider};
use decision_loop::step::GoalStep;
use provider_kit::App;
use tools::ask::AskTool;

use crate::agents::inbox::MainInbox;
use crate::agents::kit::{Kit, Own, Tools};
use crate::agents::{AgentsTool, Delegation, Registry};
use crate::brake::{Brake, BrakedDecider};
use crate::checkpoints::Checkpoints;
use crate::gate::{Stoppable, Tty};
use crate::instructions;
use crate::interrupt::Interrupt;
use crate::jobs::{self, Jobs, JobsTool};
use crate::mode::{Mode, ModeCell, ModeGate};
use crate::output::{Output, lock};
use crate::tui::asking::{UiGate, UiPerson};
use crate::windows;

/// Who calls the providers: attribution, the sign-in pages, the folder sign-ins are saved in.
pub const APP: App = App {
    name: "llx",
    slug: "llx",
    version: env!("CARGO_PKG_VERSION"),
};

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
