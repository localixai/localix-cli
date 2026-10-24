//! A session's interface and its server: the terminal's colours asked first, then in this
//! process the interface over channels to a server on a thread; the daemon serves each socket's
//! client with the same server (`serving`).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use client::Client;
use protocol::{Place, ToClient, ToServer};
use tools::workspace::Workspace;

use super::{Attached, NO_INTERFACE, RUN_EXEC};
use crate::commands::repl;
use crate::desk;
use crate::error::Failure;
use crate::interrupt;
use crate::mode::Mode;
use crate::server::host::Host;
use crate::server::{Peer, Ui, serve};
use crate::tui;

/// The terminal's colours, asked before the interface reads the keys (which would take the
/// terminal's answers), only where terminal windows will show them (`windowed`); and xterm.js for
/// those windows, once: until it is kept they show a grid.
#[cfg(unix)]
pub(super) fn colours(windowed: bool) -> Option<protocol::Theme> {
    if crate::xterm::wanted(true, windowed, |name| std::env::var(name).ok()) {
        crate::xterm::prefetch();
    }
    windowed.then(crate::colours::ask).flatten()
}

#[cfg(not(unix))]
pub(super) fn colours(_windowed: bool) -> Option<protocol::Theme> {
    None
}

/// A session takes the terminal for its interface, its server in this process; one that cannot
/// draw there (no size to draw at) is a usage error, as having no terminal is. The place the
/// interface attached from comes with it.
pub(super) fn attach(
    root: &Workspace,
    theme: Option<protocol::Theme>,
) -> Result<(Attached, Place), Failure> {
    let why = |error| Failure::Usage(format!("{NO_INTERFACE} ({error}): {RUN_EXEC}"));
    #[allow(
        clippy::disallowed_methods,
        reason = "the in-process transport: the server never waits on the interface (W6), the \
                  interface never on the desk; one frame per thing done or key pressed"
    )]
    let ((to_server, incoming), (out, from_server)) = (channel(), channel());
    let (peer, requests) = serving(incoming, out, None).map_err(why)?;
    let connect: tui::Connect =
        Box::new(move |deliver| Client::in_process(to_server, from_server, deliver));
    let running = tui::start(tui::Start {
        connect,
        root: root.root(),
        theme,
        build: String::new(),
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
        running: Some(running),
        ui: Ui::new(peer),
        requests,
        arrivals: None,
    };
    Ok((attached, place))
}

/// A server for one interface, on a thread of its own: what it serves the sessions, and what the
/// interface asks of the desk. `host`: the daemon it is part of, when it is one.
pub(crate) fn serving(
    incoming: Receiver<ToServer>,
    out: Sender<ToClient>,
    host: Option<Arc<Host>>,
) -> std::io::Result<(Arc<Peer>, Receiver<desk::Request>)> {
    #[allow(
        clippy::disallowed_methods,
        reason = "the desk's requests: one per thing the person asks of it, never waited on"
    )]
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
        // Only a daemon keeps the work once the interface leaves; in process it ends with it.
        background: host.is_some(),
        host,
    };
    let server = std::thread::Builder::new().name("llx-serve".into());
    server.spawn(move || serve::serve(&incoming, &served, &desk, &info))?;
    Ok((peer, requests))
}
