//! A run, as a session at the terminal or one `exec` task: all is checked and read on the main
//! thread first (workspace, schema, key, prompt), then the session is composed, with the browser's
//! event loop on the main thread where a window can be shown. A session at the terminal is the
//! daemon's (`llx serve`, docs/daemon.md) unless `--no-daemon`: this process is then its client,
//! and the daemon runs the same code for it (`At::Served`). Each task is run, reported, recorded
//! and its terminal sessions closed; Ctrl-C or closing the window stops it (see `interrupt`), not
//! the process.

mod interface;
#[cfg(unix)]
mod remote;
mod task;

use std::io::IsTerminal;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::Receiver;

use contracts::history::{History, Session};
use protocol::Place;
use tools::browser::Browser;
use tools::workspace::Workspace;

use crate::attach;
use crate::choice;
use crate::cli::RunOptions;
use crate::commands::sessions;
use crate::compose::{Seat, Setup, Shared, compose, lane};
use crate::context::{self, Environment};
use crate::desk;
use crate::error::Failure;
use crate::gate::Tty;
use crate::input::{self, Prompt};
use crate::interrupt;
use crate::mode::{self, Mode};
use crate::server::{Ui, place};
use crate::tui;
use crate::windows;

pub(crate) use self::interface::{Left, info, serving};
pub(crate) use super::ended::{Ended, since};

/// The workspace `options` name (`-C`, else the current directory), with what is llx's own off
/// limits to the tools (`place::at`).
pub(crate) fn root(options: &RunOptions) -> Result<Workspace, Failure> {
    let root = options.cd.as_deref().unwrap_or(Path::new("."));
    place::at(root).map_err(|why| Failure::Usage(format!("-C {}: {why}", root.display())))
}

/// How a run takes its tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A session at the terminal: the prompt, when given, is its first message.
    Session,
    /// One task, then exit (`exec`, -p).
    Exec,
}

/// What a run does once composed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Work {
    /// One task, then exit.
    One(String),
    /// A session in the interface, after a first message when there is one.
    Session(Option<String>),
}

/// What a session needs that `exec` does not, as a usage error.
const SESSION_TERMINAL: &str = "a session needs a terminal on stdin: run one task with \
`llx exec \"<task>\"` (or -p), or one more in a saved session with `llx exec resume`";
const NO_INTERFACE: &str = "a session's interface cannot start at this terminal";
const RUN_EXEC: &str = "run one task with `llx exec \"<task>\"` (or -p)";
const ONE_TASK: &str = "--json, --schema and -o answer one task: run it with `llx exec` (or -p)";
const EXEC_PROMPT: &str = "exec runs one task: give it as the prompt, or `-` to read it from stdin";

/// The work `kind` asks of `prompt`; stdin is read only for one task (a session's is the
/// terminal its interface reads).
fn work(prompt: Option<&str>, kind: Kind) -> Result<Work, Failure> {
    match kind {
        Kind::Exec => match input::prompt(prompt)? {
            Prompt::One(task) => Ok(Work::One(task)),
            Prompt::Line => Err(Failure::Usage(EXEC_PROMPT.into())),
        },
        Kind::Session => Ok(Work::Session(prompt.map(str::to_string))),
    }
}

/// Whether a session can run: at a terminal, with no option that answers one task.
pub(crate) fn session_allowed(
    prompt: Option<&str>,
    options: &RunOptions,
    terminal: bool,
) -> Result<(), Failure> {
    if options.json || options.schema.is_some() || options.output_last_message.is_some() {
        return Err(Failure::Usage(ONE_TASK.into()));
    }
    if !terminal || prompt == Some("-") {
        return Err(Failure::Usage(SESSION_TERMINAL.into()));
    }
    Ok(())
}

/// Where a run's session is shown.
pub(crate) enum At {
    /// At this process's terminal: through the daemon, else in this process.
    Terminal,
    /// Served by the daemon to the client that attached from its place.
    #[cfg_attr(not(unix), expect(dead_code, reason = "the daemon is unix only"))]
    Served(Served),
}

/// A session the daemon serves: where its client is, the client's interface and what it asks of
/// the desk, the daemon's window, and the lanes it tells what the person does there.
#[cfg_attr(not(unix), expect(dead_code, reason = "the daemon is unix only"))]
pub(crate) struct Served {
    pub place: Place,
    pub ui: Ui,
    pub requests: Receiver<desk::Request>,
    pub browser: Option<Browser>,
    pub arrivals: Arc<crate::arrivals::Lanes>,
    /// Its desk's terminal sessions' numbering, which its server counts.
    pub terminals: Arc<AtomicU32>,
}

impl Served {
    /// Its client's interface as a session's, with where the client is, and the window.
    fn attached(self) -> (Attached, Place, Option<Browser>) {
        let attached = Attached {
            running: None,
            ui: self.ui,
            requests: self.requests,
            arrivals: Some(self.arrivals),
            terminals: self.terminals,
        };
        (attached, self.place, self.browser)
    }
}

/// Run `prompt` with `options` as `kind`, in `resumed` when a session goes on, shown `at`.
pub(crate) fn run(
    prompt: Option<&str>,
    options: RunOptions,
    resumed: Option<Session>,
    kind: Kind,
    at: At,
) -> Result<i32, Failure> {
    let mut workspace = root(&options)?;
    let schema = options.schema.as_deref().map(input::schema).transpose()?;
    // A picture asked for that cannot be attached stops the run before a model is paid for.
    for path in &options.images {
        attach::read(path).map_err(|why| Failure::Usage(format!("--image {why}")))?;
    }
    let served = match at {
        At::Terminal => None,
        At::Served(served) => Some(served),
    };
    if kind == Kind::Session && served.is_none() {
        session_allowed(prompt, &options, std::io::stdin().is_terminal())?;
    }
    // Served, the session thinks with its client's keys, not the daemon's.
    let vars = served.as_ref().map(|served| served.place.env.as_slice());
    let choice = choice::resolve(&options.models, choice::saved(), vars)?;
    // A session starts signed out too, so that /login can sign in; one task cannot.
    if kind == Kind::Exec && choice.provider.signed_in().is_none() {
        return Err(Failure::SignedOut(choice.provider.how_to_sign_in()));
    }
    let work = work(prompt, kind)?;
    let mode = Mode::choose(options.mode, options.yes);
    // The daemon's client has the terminal the person is asked at.
    let (terminal, asks) = (served.is_some() || Tty::available(), kind == Kind::Session);
    let set = |name: &str| match &served {
        Some(served) => served.place.env.iter().any(|(set, _)| set == name),
        None => std::env::var_os(name).is_some(),
    };
    let shown = windows::shown(&options, asks, mode, windows::remote(set));
    let browser =
        (served.as_ref()).map_or_else(tools::browser::has_display, |s| s.browser.is_some());
    // A session's interface attaches first: its sessions work where it is.
    let (ready, place, hosted) = match (work, served) {
        (Work::One(_), Some(_)) => return Err(Failure::Usage(RUN_EXEC.into())),
        (Work::One(task), None) => (Ready::One(task), None, None),
        (Work::Session(first), Some(served)) => {
            let (attached, place, browser) = served.attached();
            (Ready::Session(first, attached), Some(place), Some(browser))
        }
        (Work::Session(first), None) => {
            let theme = interface::colours(browser && shown);
            #[cfg(unix)]
            if !options.no_daemon {
                match remote::run(&workspace, theme) {
                    Ok(code) => return Ok(code),
                    Err(why) => crate::error::note(&format!("llx: {why}: {IN_PROCESS}\n")),
                }
            }
            let (attached, place) = interface::attach(&workspace, theme)?;
            (Ready::Session(first, attached), Some(place), None)
        }
    };
    if let Some(place) = &place {
        workspace = place::workspace(place).map_err(Failure::Usage)?;
    }
    let environment = Environment {
        root: workspace.root(),
        mode: &mode::describe(mode, terminal, asks),
        browser,
        now: session::now(),
    };
    let context = context::gather(&environment, resumed.as_ref());
    let fresh = context::gather(&environment, None);
    let history = sessions::store();
    let mut session = resumed.unwrap_or_else(|| history.start());
    if session.workspace.is_empty() {
        session.workspace = workspace.root().display().to_string();
    }
    let setup = Setup {
        options,
        choice,
        workspace,
        context,
        fresh,
        schema,
        mode,
        terminal,
        asks,
        history,
        session,
        ui: None,
        shown,
        #[cfg(unix)]
        theme: place.as_ref().and_then(place::theme),
    };
    #[cfg(not(unix))]
    let _ = place;
    let work = move |browser| ready.run(setup, browser);
    // The daemon's window, on its main thread already.
    if let Some(browser) = hosted {
        return Ok(work(browser));
    }
    interrupt::install();
    if !browser {
        return Ok(work(None));
    }
    tools::browser::host(interrupt::window_closed, move |b| work(Some(b)))
}

/// Said when the daemon cannot serve this session.
const IN_PROCESS: &str = "this session runs in this process";

/// A run's work with what it runs in: a session with its interface, or one task.
enum Ready {
    Session(Option<String>, Attached),
    One(String),
}

/// A session's interface attached to its server: the terminal's (none when the daemon serves a
/// client), the sessions' port, what the person asks of the desk, and the lanes the window's
/// events go to when they are the daemon's.
struct Attached {
    running: Option<tui::Running>,
    ui: Ui,
    requests: Receiver<desk::Request>,
    arrivals: Option<Arc<crate::arrivals::Lanes>>,
    /// The numbering of the desk's terminal sessions, which its server counts.
    terminals: Arc<AtomicU32>,
}

impl Ready {
    /// Run it with `browser`; the exit code.
    fn run(self, setup: Setup, browser: Option<Browser>) -> i32 {
        match self {
            // Each lane of the session is a session of its own, on its own thread.
            Self::Session(first, attached) => {
                let Attached {
                    running,
                    ui,
                    requests,
                    arrivals,
                    terminals,
                } = attached;
                let (mut shared, seat) = match arrivals {
                    Some(lanes) => setup.split_into(browser, lanes),
                    None => setup.split(browser),
                };
                shared.terminal_ids = terminals;
                let seat = Seat {
                    ui: Some(ui.clone()),
                    ..seat
                };
                let start = |shared: &Shared, seat, work: desk::Work| lane(shared, seat, work);
                // Lanes that do not stop in time: the terminal goes back, and the process ends;
                // in the daemon, which serves other terminals, this desk's terminal sessions are
                // killed and the client is told the session is over: its lanes end as they can,
                // with nobody waiting on them.
                let in_process = running.is_some();
                let (told, terminals) = (ui.clone(), Arc::clone(&shared.terminal_ids));
                let quit = move || match in_process {
                    true => {
                        tui::restore();
                        interrupt::quit_now();
                    }
                    false => {
                        tools::shell::kill_with(&terminals);
                        told.send(protocol::Event::Quit);
                    }
                };
                let code = desk::run(&shared, &ui, requests, seat, first, &start, &quit);
                drop(running);
                code
            }
            Self::One(task) => compose(setup, browser, |rig| {
                rig.attached = rig.options.images.clone();
                rig.task(&task)
            }),
        }
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
