//! `llx serve`: the daemon (docs/daemon.md). It takes the lock of its config, binds the socket,
//! says it is ready, and runs the window's event loop on the main thread; each client that
//! attaches gets a desk of its own, built as a terminal's session is (`run::run`, `At::Served`)
//! from the place and the command line the client gives. It never ends by itself:
//! `llx serve stop` ends it. `llx serve status` says what it is.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::{Duration, Instant};

use clap::Parser;
use protocol::{Event, Place, ToServer};
use tools::browser::Browser;

use crate::arrivals::Lanes;
use crate::cli::{Cli, Command, ServeAction};
use crate::commands::run::{self, At, Served};
use crate::daemon;
use crate::desk::Request;
use crate::error::{Failure, print};
use crate::exit;
use crate::interrupt;
use crate::server::Ui;
use crate::server::host::{Host, Kept};
use crate::server::serve;
use crate::server::socket::{self, Connection};
use crate::server::toolbox;

/// How long the daemon waits for its lanes to end once stopped; and for another daemon's lock
/// that is being let go (one that was just asked to end).
const ENDING: Duration = Duration::from_secs(2);

pub(crate) fn serve(action: Option<&ServeAction>) -> Result<i32, Failure> {
    let config = daemon::config().map_err(Failure::Environment)?;
    let path = daemon::socket_path(&config);
    match action {
        None => host(&config, daemon::own_socket(&config)),
        Some(ServeAction::Status) => match daemon::status(&path, ENDING) {
            Some(found) => {
                let daemon::Status {
                    build,
                    pid,
                    attached,
                    lanes,
                    working,
                    kept,
                    jobs,
                } = found;
                print(&format!(
                    "llx serve: running, pid {pid}, build {build}, {attached} attached, \
                     {lanes} sessions open, {working} working, {kept} in the background, \
                     {jobs} jobs\n"
                ))?;
                Ok(exit::OK)
            }
            None => print("llx serve: not running\n").map(|()| exit::FAILED),
        },
        Some(ServeAction::Stop) => match daemon::stop(&path) {
            true => print("llx serve: stopped\n").map(|()| exit::OK),
            false => Err(Failure::Environment("the daemon did not stop".into())),
        },
    }
}

/// The daemon itself, until it is asked to end.
fn host(config: &Path, path: PathBuf) -> Result<i32, Failure> {
    let lock = locked(config)?.ok_or_else(|| {
        Failure::Environment(format!(
            "another daemon serves {} (llx serve status)",
            config.display()
        ))
    })?;
    let failed = |error: std::io::Error| Failure::Environment(format!("llx serve: {error}"));
    let listener = socket::bind(&path).map_err(failed)?;
    let host = Host::new(daemon::build());
    let pid = std::process::id();
    print(&format!("{} (pid {pid})\n", daemon::READY_LINE))?;
    daemon::quiet();
    interrupt::install();
    crate::providers::refresh_catalog();
    let work = move |browser: Option<Browser>| {
        let _lock = lock;
        let lanes = crate::arrivals::listening(browser.as_ref());
        let served = Arc::clone(&host);
        let each = move |connection| attach(connection, &served, browser.clone(), &lanes);
        if socket::accept(listener, each).is_err() {
            return exit::ENVIRONMENT;
        }
        host.wait();
        // Every task stops and is saved; the terminal sessions go with the process.
        let _ = interrupt::PROCESS.stop();
        let until = Instant::now() + ENDING;
        while interrupt::working() > 0 && Instant::now() < until {
            backoff();
        }
        tools::shell::kill_all_now();
        let _ = std::fs::remove_file(&path);
        exit::OK
    };
    if !tools::browser::has_display() {
        return Ok(work(None));
    }
    Ok(tools::browser::host(
        interrupt::window_closed_served,
        move |browser| work(Some(browser)),
    ))
}

/// The lock, waiting a little for a daemon that is ending to let it go.
fn locked(config: &Path) -> Result<Option<daemon::Lock>, Failure> {
    let until = Instant::now() + ENDING;
    loop {
        let lock = daemon::lock(config).map_err(|error| Failure::Environment(error.to_string()))?;
        if lock.is_some() || Instant::now() >= until {
            return Ok(lock);
        }
        backoff();
    }
}

#[allow(
    clippy::disallowed_methods,
    reason = "a backoff: neither a lock let go nor a task's end has an event to wait on here"
)]
fn backoff() {
    std::thread::sleep(Duration::from_millis(50));
}

/// One client: back to the sessions moved to the background where it is, else its server and,
/// once it says where it is, its desk until the desk ends. A client that only asks (`status`,
/// `stop`) says nothing of where it is, and gets no desk.
fn attach(connection: Connection, host: &Arc<Host>, browser: Option<Browser>, lanes: &Arc<Lanes>) {
    let Connection { incoming, out } = connection;
    let Ok(first) = incoming.recv() else {
        return;
    };
    // A tool client (`llx mcp`) gets a toolbox, not a desk (W9).
    if toolbox::wanted(&first) {
        let hosted = toolbox::Hosted {
            host: Some(Arc::clone(host)),
            browser,
            lanes: Arc::clone(lanes),
            terminals: Arc::default(),
        };
        return toolbox::serve(first, &incoming, &out, &hosted);
    }
    if let Some(kept) = rejoinable(&first, host) {
        return rejoin(first, &incoming, out, kept, host);
    }
    // An interface is attached while its connection lasts; its desk may outlive it, kept.
    let attached = placed(&first).is_some().then(|| host.attach());
    // Where the client is, once it says: the work it moves to the background is kept there.
    let at: Arc<std::sync::Mutex<Option<String>>> = Arc::default();
    let (theirs, daemon) = (Arc::clone(&at), Arc::clone(host));
    let terminals: Arc<AtomicU32> = Arc::default();
    let kept_terminals = Arc::clone(&terminals);
    let ended: run::Left = Box::new(move |kept, peer, desk| {
        drop(attached);
        let cwd = crate::output::lock(&theirs).clone();
        match (kept, cwd) {
            (true, Some(cwd)) => daemon.keep(Kept {
                cwd,
                peer,
                desk,
                terminals: kept_terminals,
            }),
            _ => {
                let _ = desk.send(Request::Gone);
            }
        }
    });
    let daemon = Some(Arc::clone(host));
    let numbering = Arc::clone(&terminals);
    let served = run::serving(Some(first), incoming, out, daemon, numbering, ended);
    let Ok((peer, requests)) = served else {
        return;
    };
    let place = loop {
        match requests.recv() {
            Ok(Request::Attach(place)) => break place,
            Ok(_) => {}
            Err(_gone) => return,
        }
    };
    *crate::output::lock(&at) = Some(place.cwd.clone());
    let _forgotten = Forget(Arc::clone(host), Arc::clone(&peer));
    let ui = Ui::new(peer);
    let arrivals = Arc::clone(lanes);
    let told = ui.clone();
    let served = Served {
        place,
        ui,
        requests,
        browser,
        arrivals,
        terminals,
    };
    if let Err(why) = session(served) {
        told.send(Event::Error(format!("llx: {why}")));
        told.send(Event::Quit);
    }
}

/// The place and build an interface's handshake gives.
fn placed(first: &ToServer) -> Option<(&Place, &str)> {
    match first {
        ToServer::Call {
            command:
                protocol::Command::Initialize {
                    place: Some(place),
                    build,
                    ..
                },
            ..
        } => Some((place, build)),
        _ => None,
    }
}

/// The sessions moved to the background where `first`'s client is, of its build, when its
/// command line asks for nothing but a session: a task, `resume`, a model or any other option is
/// a new desk's, built from it, and the sessions kept wait for a plain `llx` there. They keep the
/// place they were opened in (its environment, its colours): their work already runs there.
fn rejoinable(first: &ToServer, host: &Host) -> Option<Kept> {
    let (place, build) = placed(first)?;
    let plain = place.args.is_empty() && build == host.build;
    plain.then(|| host.take(&place.cwd)).flatten()
}

/// The client back to `kept`: given what its sessions did meanwhile, then served; kept again if
/// it moves them to the background once more, else their work stops.
fn rejoin(
    first: ToServer,
    incoming: &std::sync::mpsc::Receiver<ToServer>,
    out: std::sync::mpsc::Sender<protocol::ToClient>,
    kept: Kept,
    host: &Arc<Host>,
) {
    let _attached = host.attach();
    let info = run::info(Some(Arc::clone(host)), Arc::clone(&kept.terminals));
    match serve::rejoin(first, incoming, &kept.peer, out, &kept.desk, &info) {
        true => host.keep(kept),
        false => {
            let _ = kept.desk.send(Request::Gone);
        }
    }
}

/// A desk's end, however it ends: nothing of it is kept for a later client.
struct Forget(Arc<Host>, Arc<crate::server::Peer>);

impl Drop for Forget {
    fn drop(&mut self) {
        self.0.forget(&self.1);
    }
}

/// The session the client's command line asks for, where it is.
fn session(served: Served) -> Result<i32, Failure> {
    let place = &served.place;
    let words = std::iter::once("llx".to_string()).chain(place.args.iter().cloned());
    let mut cli = Cli::try_parse_from(words).map_err(|error| Failure::Usage(error.to_string()))?;
    if cli.print || !matches!(cli.command, None | Some(Command::Resume(_))) {
        return Err(Failure::Usage("the daemon serves sessions only".into()));
    }
    // Its paths are the client's: the directory it works in, the pictures it attached.
    let options = cli.options();
    options.cd = Some(PathBuf::from(&place.cwd));
    // The model the client's environment names, not what the daemon's had.
    for (var, value) in crate::server::place::chosen(place) {
        let models = &mut options.models;
        match var {
            "LLX_PROVIDER" => models.provider = Some(value),
            "LLX_MODEL" => models.model = Some(value),
            "LLX_REASONING" => models.reasoning = Some(value),
            _ => models.decision_model = value,
        }
    }
    for image in &mut options.images {
        *image = Path::new(&place.dir).join(&*image);
    }
    crate::commands::dispatch(cli, At::Served(served))
}
