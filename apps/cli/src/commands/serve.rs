//! `llx serve`: the daemon (docs/daemon.md). It takes the lock of its config, binds the socket,
//! says it is ready, and runs the window's event loop on the main thread; each client that
//! attaches gets a desk of its own, built as a terminal's session is (`run::run`, `At::Served`)
//! from the place and the command line the client gives. It never ends by itself:
//! `llx serve stop` ends it. `llx serve status` says what it is.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use protocol::{Event, Place};
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
use crate::server::host::Host;
use crate::server::socket::{self, Connection};

/// How long the daemon waits for its lanes to end once stopped; and for another daemon's lock
/// that is being let go (one that was just asked to end).
const ENDING: Duration = Duration::from_secs(2);

pub(crate) fn serve(action: Option<&ServeAction>) -> Result<i32, Failure> {
    let config = daemon::config().map_err(Failure::Environment)?;
    let path = daemon::socket_path(&config);
    match action {
        None => host(&config, path),
        Some(ServeAction::Status) => match daemon::status(&path, ENDING) {
            Some(found) => {
                let daemon::Status {
                    build,
                    pid,
                    attached,
                    lanes,
                    working,
                } = found;
                print(&format!(
                    "llx serve: running, pid {pid}, build {build}, {attached} attached, \
                     {lanes} sessions open, {working} working\n"
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
    tools::browser::host(interrupt::window_closed, move |browser| work(Some(browser)))
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

/// One client: its server, then, once it says where it is, its desk until the desk ends. A
/// client that only asks (`status`, `stop`) says nothing of where it is, and gets no desk.
fn attach(connection: Connection, host: &Arc<Host>, browser: Option<Browser>, lanes: &Arc<Lanes>) {
    let Connection { incoming, out } = connection;
    let Ok((peer, requests)) = run::serving(incoming, out, Some(Arc::clone(host))) else {
        return;
    };
    let place = loop {
        match requests.recv() {
            Ok(Request::Attach(place)) => break place,
            Ok(_) => {}
            Err(_gone) => return,
        }
    };
    let _attached = host.attach();
    let ui = Ui::new(peer);
    let arrivals = Arc::clone(lanes);
    let told = ui.clone();
    if let Err(why) = session(place, ui, requests, browser, arrivals) {
        told.send(Event::Error(format!("llx: {why}")));
        told.send(Event::Quit);
    }
}

/// The session the client's command line asks for, where it is.
fn session(
    place: Place,
    ui: Ui,
    requests: std::sync::mpsc::Receiver<Request>,
    browser: Option<Browser>,
    arrivals: Arc<Lanes>,
) -> Result<i32, Failure> {
    let words = std::iter::once("llx".to_string()).chain(place.args.iter().cloned());
    let mut cli = Cli::try_parse_from(words).map_err(|error| Failure::Usage(error.to_string()))?;
    if cli.print || !matches!(cli.command, None | Some(Command::Resume(_))) {
        return Err(Failure::Usage("the daemon serves sessions only".into()));
    }
    // Its paths are the client's: the directory it works in, the pictures it attached.
    let options = cli.options();
    options.cd = Some(PathBuf::from(&place.cwd));
    for image in &mut options.images {
        *image = Path::new(&place.dir).join(&*image);
    }
    let served = Served {
        place,
        ui,
        requests,
        browser,
        arrivals,
    };
    crate::commands::dispatch(cli, At::Served(served))
}
