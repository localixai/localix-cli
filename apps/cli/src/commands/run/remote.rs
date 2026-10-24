//! A session at the terminal as the daemon's client (docs/daemon.md, "Lifecycle"): the daemon
//! found, else started; then the interface over its socket, until the session is over. What
//! fails before the interface draws leaves the terminal as it was, for a session in process.

use tools::workspace::Workspace;

use crate::{daemon, exit, interrupt, tui};

/// The session in the daemon, this terminal its interface; why not, when it cannot be.
pub(super) fn run(root: &Workspace, theme: Option<protocol::Theme>) -> Result<i32, String> {
    let config = daemon::config()?;
    let path = daemon::reach(&config)?;
    let connect: tui::Connect = Box::new(move |deliver| client::Client::connect(&path, deliver));
    let start = tui::Start {
        connect,
        root: root.root(),
        theme,
        build: daemon::build(),
        quit_now: interrupt::quit_now,
    };
    let running =
        tui::start(start).map_err(|error| format!("the daemon did not attach ({error})"))?;
    running.wait();
    Ok(exit::OK)
}
