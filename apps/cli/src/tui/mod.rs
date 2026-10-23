//! The terminal interface: what a person sees and types. Finished blocks go into the terminal's
//! own scrollback once (so scrolling, selecting and copying stay the terminal's); only a small
//! live area at the bottom is redrawn. Every block is a pure function from state and width to
//! styled lines, which the snapshot tests pin.
//!
//! One thread owns the terminal: it reads the keys (from a reader thread), takes the server's
//! frames through a [`Client`], and draws. The sessions run in the server; the interface knows
//! them only by what the protocol carries (ARCHITECTURE.md L7).

pub(crate) mod agents;
pub(crate) mod app;
pub(crate) mod cells;
mod clipboard;
pub(crate) mod composer;
mod drawing;
pub(crate) mod event;
pub(crate) mod keys;
mod list;
pub(crate) mod live;
pub(crate) mod markdown;
pub(crate) mod mention;
mod opening;
pub(crate) mod questions;
pub(crate) mod queue;
mod quitting;
pub(crate) mod screen;
mod search;
pub(crate) mod select;
mod sessions;
mod spend;
mod steer;
pub(crate) mod style;
mod terminal;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod tool_cell;
pub(crate) mod tree;
pub(crate) mod views;
pub(crate) mod wrap;

use std::io::{self, Write};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use client::Client;
use protocol::{Command, Done, Event, Place, ToClient, ToServer};

use self::app::App;
use self::event::Input;
pub(crate) use self::terminal::restore;

/// The interface running; dropping it gives the terminal back.
pub(crate) struct Running {
    /// Where the interface's thread takes what comes: told here the session is over.
    inbox: Sender<Input>,
    thread: Option<JoinHandle<()>>,
}

/// What the interface is given: the server's ends of the frames, what `/` offers, the
/// directory for `@` (where the sessions it opens work), the terminal's colours, and how to end
/// the process at once.
pub(crate) struct Start<'a> {
    pub to_server: Sender<ToServer>,
    pub from_server: Receiver<ToClient>,
    pub commands: Vec<(String, String)>,
    pub root: &'a std::path::Path,
    pub theme: Option<protocol::Theme>,
    pub quit_now: fn() -> !,
}

/// Takes the terminal, attaches to the server and starts drawing.
pub(crate) fn start(start: Start) -> io::Result<Running> {
    let (width, height) = crossterm::terminal::size()?;
    crossterm::terminal::enable_raw_mode()?;
    let mut out = io::stdout();
    // Nowhere to draw (stdout's reader gone): the terminal goes back as it was.
    if let Err(error) = out
        .write_all(terminal::ENTER.as_bytes())
        .and_then(|()| out.flush())
    {
        restore();
        return Err(error);
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
    match attach(start, width, height) {
        Ok(running) => Ok(running),
        Err(error) => {
            restore();
            Err(error)
        }
    }
}

/// The key reader, the client and the interface's thread, once the terminal is the interface's.
fn attach(start: Start, width: u16, height: u16) -> io::Result<Running> {
    #[allow(
        clippy::disallowed_methods,
        reason = "the interface must never make the server or the keys wait: it takes \
                  everything that came before each frame it draws"
    )]
    let (inbox, taken) = channel();
    let keys = inbox.clone();
    let reader = std::thread::Builder::new().name("llx-keys".into());
    reader.spawn(move || terminal::read_keys(&keys))?;
    let frames = inbox.clone();
    let client = Client::in_process(start.to_server, start.from_server, move |frame| {
        let _ = frames.send(Input::Frame(frame));
    })?;
    let hello = Command::Initialize {
        version: protocol::VERSION,
        place: Some(here(start.root, start.theme)),
    };
    let Ok(Done::Initialized { max_lanes, .. }) = client.call(hello, Duration::from_secs(5)) else {
        return Err(io::Error::other("the server did not attach the interface"));
    };
    let mut app = App::new(usize::from(width), usize::from(height), start.commands);
    app.files = mention::Files::new(Some(start.root.to_path_buf()));
    app.shells = tools::shell::running_anywhere;
    app.max_lanes = max_lanes;
    let seat = drawing::Seat {
        client,
        quit_now: start.quit_now,
    };
    let drawing = std::thread::Builder::new().name("llx-interface".into());
    let thread = drawing.spawn(move || drawing::draw(app, &taken, &seat))?;
    Ok(Running {
        inbox,
        thread: Some(thread),
    })
}

/// Where this interface is: the sessions it opens work there, and their commands run with its
/// environment (a variable that is not text is left out).
fn here(root: &std::path::Path, theme: Option<protocol::Theme>) -> Place {
    let env = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)));
    Place {
        cwd: root.display().to_string(),
        env: env.collect(),
        theme,
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let event = Event::Quit;
        let _ = (self.inbox).send(Input::Frame(ToClient::Event {
            session: None,
            event,
        }));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        restore();
    }
}
