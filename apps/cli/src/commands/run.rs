//! A run, as a session at the terminal or one `exec` task: all is checked and read on the main
//! thread first (workspace, schema, key, prompt), then the session is composed, with the browser's
//! event loop on the main thread where a window can be shown. Each task is run, reported, recorded
//! and its terminal sessions closed; Ctrl-C or closing the window stops it (see `interrupt`), not
//! the process.

mod task;

use std::io::IsTerminal;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use contracts::history::{History, Session};
use protocol::Place;
use tools::browser::Browser;
use tools::workspace::Workspace;

use crate::attach;
use crate::choice;
use crate::cli::RunOptions;
use crate::commands::{repl, sessions};
use crate::compose::{Seat, Setup, Shared, compose, lane};
use crate::context::{self, Environment};
use crate::desk;
use crate::error::Failure;
use crate::gate::Tty;
use crate::input::{self, Prompt};
use crate::interrupt;
use crate::mode::{self, Mode};
use crate::server::{Peer, Ui, place, serve};
use crate::tui;
use crate::windows;

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

/// Run `prompt` with `options` as `kind`, in `resumed` when a session goes on.
pub(crate) fn run(
    prompt: Option<&str>,
    options: RunOptions,
    resumed: Option<Session>,
    kind: Kind,
) -> Result<i32, Failure> {
    let mut workspace = root(&options)?;
    let schema = options.schema.as_deref().map(input::schema).transpose()?;
    // A picture asked for that cannot be attached stops the run before a model is paid for.
    for path in &options.images {
        attach::read(path).map_err(|why| Failure::Usage(format!("--image {why}")))?;
    }
    if kind == Kind::Session {
        session_allowed(prompt, &options, std::io::stdin().is_terminal())?;
    }
    let choice = choice::resolve(&options.models, choice::saved())?;
    // A session starts signed out too, so that /login can sign in; one task cannot.
    if kind == Kind::Exec && choice.provider.signed_in().is_none() {
        return Err(Failure::SignedOut(choice.provider.how_to_sign_in()));
    }
    let work = work(prompt, kind)?;
    let mode = Mode::choose(options.mode, options.yes);
    let (terminal, asks) = (Tty::available(), kind == Kind::Session);
    let browser = tools::browser::has_display();
    let remote = windows::remote(|name| std::env::var_os(name).is_some());
    let shown = windows::shown(&options, asks, mode, remote);
    // A session's interface attaches first: its sessions work where it is.
    let (ready, place) = match work {
        Work::Session(first) => {
            let (attached, place) = interface(&workspace, browser && shown)?;
            workspace = place::workspace(&place).map_err(Failure::Usage)?;
            (Ready::Session(first, attached), Some(place))
        }
        Work::One(task) => (Ready::One(task), None),
    };
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
    interrupt::install();
    if !browser {
        return Ok(work(None));
    }
    tools::browser::host(interrupt::window_closed, move |b| work(Some(b)))
}

/// A run's work with what it runs in: a session with its interface, or one task.
enum Ready {
    Session(Option<String>, Attached),
    One(String),
}

/// A session's interface attached to its server: the terminal's, the sessions' port, and what
/// the person asks of the desk.
struct Attached {
    running: tui::Running,
    ui: Ui,
    requests: Receiver<desk::Request>,
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
                } = attached;
                let (shared, seat) = setup.split(browser);
                let seat = Seat {
                    ui: Some(ui.clone()),
                    ..seat
                };
                let start = |shared: &Shared, seat, work: desk::Work| lane(shared, seat, work);
                // Lanes that do not stop in time: the terminal goes back, and the process ends.
                let quit = || {
                    tui::restore();
                    interrupt::quit_now();
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

/// A session takes the terminal for its interface; one that cannot draw there (no size to draw
/// at) is a usage error, as having no terminal is. `windowed`: terminal windows will show. The
/// place the interface attached from comes with it.
fn interface(root: &Workspace, windowed: bool) -> Result<(Attached, Place), Failure> {
    // Before the interface reads the keys, which would take the terminal's answers; only where
    // terminal windows will show them.
    #[cfg(unix)]
    let theme = windowed.then(crate::colours::ask).flatten();
    #[cfg(not(unix))]
    let theme = {
        let _ = windowed;
        None
    };
    // xterm.js for the terminal windows, once: until it is kept they show a grid.
    #[cfg(unix)]
    if crate::xterm::wanted(true, windowed, |name| std::env::var(name).ok()) {
        crate::xterm::prefetch();
    }
    let why = |error| Failure::Usage(format!("{NO_INTERFACE} ({error}): {RUN_EXEC}"));
    let Serving {
        peer,
        requests,
        to_server,
        from_server,
    } = serving().map_err(why)?;
    let running = tui::start(tui::Start {
        to_server,
        from_server,
        root: root.root(),
        theme,
        quit_now: interrupt::quit_now,
    });
    let running = running.map_err(why)?;
    // Said before the interface was told it is attached: the first of the desk's requests.
    let Ok(desk::Request::Attach(place)) = requests.try_recv() else {
        return Err(Failure::Usage(format!("{NO_INTERFACE}: it gave no place")));
    };
    // A newer model catalog, for the next session: nothing here waits on it.
    crate::providers::refresh_catalog();
    let attached = Attached {
        running,
        ui: Ui::new(peer),
        requests,
    };
    Ok((attached, place))
}

/// The server, in this process: what it serves the interface on its own thread, the desk's
/// requests, and the interface's ends of the frames.
#[allow(
    clippy::disallowed_methods,
    reason = "the in-process transport: the server never waits on the interface (W6), the \
              interface never on the desk; one frame per thing done or key pressed"
)]
fn serving() -> std::io::Result<Serving> {
    let (to_server, incoming) = channel();
    let (out, from_server) = channel();
    let (desk, requests) = channel();
    let peer = Peer::new(out);
    let served = Arc::clone(&peer);
    let info = serve::Info {
        max_lanes: desk::MAX_LANES,
        shells: tools::shell::running_anywhere,
        commands: repl::commands(),
        modes: Mode::ALL
            .map(|mode| (mode.name().into(), mode.hint()))
            .to_vec(),
    };
    let server = std::thread::Builder::new().name("llx-serve".into());
    server.spawn(move || serve::serve(&incoming, &served, &desk, &info))?;
    Ok(Serving {
        peer,
        requests,
        to_server,
        from_server,
    })
}

/// The server running: the sessions' side of it, the desk's requests, and the interface's ends.
struct Serving {
    peer: Arc<Peer>,
    requests: Receiver<desk::Request>,
    to_server: Sender<protocol::ToServer>,
    from_server: Receiver<protocol::ToClient>,
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
