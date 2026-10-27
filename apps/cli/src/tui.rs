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
pub(crate) mod attach;
pub(crate) mod cells;
mod clipboard;
pub(crate) mod composer;
mod drawing;
pub(crate) mod event;
mod farewell;
mod here;
pub(crate) mod keys;
mod list;
pub(crate) mod live;
pub(crate) mod markdown;
pub(crate) mod mention;
mod opening;
pub(crate) mod printable;
pub(crate) mod questions;
pub(crate) mod queue;
mod quitting;
mod route;
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
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use client::Client;
use protocol::{Command, Done, Event, Place, ToClient};

use self::app::App;
use self::event::Input;
pub(crate) use self::terminal::restore;

/// The interface running; dropping it gives the terminal back.
pub(crate) struct Running {
    /// Where the interface's thread takes what comes: told here the session is over.
    inbox: Sender<Input>,
    thread: Option<JoinHandle<Vec<ratatui::text::Line<'static>>>>,
    /// What the interface said it will print once the terminal is given back.
    farewell: Vec<ratatui::text::Line<'static>>,
}

/// Where the server's frames that are not answers go, until the server is gone.
pub(crate) type Deliver = Box<dyn Fn(ToClient) + Send>;

/// The client to the server, made with where its frames go: over channels in process, or over
/// the daemon's socket.
pub(crate) type Connect<'a> = Box<dyn FnOnce(Deliver) -> io::Result<Client> + 'a>;

/// What the interface is given: its connection to the server, the directory for `@` (where the
/// sessions it opens work), the terminal's colours, this binary's build (a daemon of another
/// refuses it), and how to end the process at once.
pub(crate) struct Start<'a> {
    pub connect: Connect<'a>,
    pub root: &'a std::path::Path,
    pub theme: Option<protocol::Theme>,
    pub build: String,
    pub quit_now: fn() -> !,
}

/// Takes the terminal, attaches to the server and starts drawing.
pub(crate) fn start(start: Start) -> io::Result<Running> {
    let (width, height) = crossterm::terminal::size()?;
    #[cfg(unix)]
    terminal::on_signals(start.quit_now);
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
    #[expect(
        clippy::disallowed_methods,
        reason = "the interface must never make the server or the keys wait: it takes \
                  everything that came before each frame it draws"
    )]
    let (inbox, taken) = channel();
    let keys = inbox.clone();
    let reader = std::thread::Builder::new().name("llx-keys".into());
    reader.spawn(move || terminal::read_keys(&keys))?;
    let gone = Gone(inbox.clone());
    let client = (start.connect)(Box::new(move |frame| {
        let _ = gone.0.send(Input::Frame(frame));
    }))?;
    let hello = Command::Initialize {
        version: protocol::VERSION,
        place: Some(here(start.root, start.theme)),
        build: start.build,
    };
    let Ok(Done::Initialized {
        max_lanes,
        commands,
        modes,
        background,
        ..
    }) = client.call(hello, drawing::ANSWER)
    else {
        return Err(io::Error::other("the server did not attach the interface"));
    };
    let mut app = App::new(usize::from(width), usize::from(height), commands);
    let files = inbox.clone();
    let arrive = move |list| {
        let _ = files.send(Input::Files(list));
    };
    app.files = mention::Files::new(start.root.to_path_buf(), Box::new(arrive));
    let asking = client.clone();
    // Asked only when the quit question needs it.
    app.work = Box::new(move || match asking.call(Command::Work, drawing::ANSWER) {
        Ok(Done::Work { running, .. }) => running,
        _ => vec![],
    });
    app.background = background;
    app.max_lanes = max_lanes;
    app.modes = modes;
    let seat = drawing::Seat {
        client,
        quit_now: start.quit_now,
    };
    let drawing = std::thread::Builder::new().name("llx-interface".into());
    let thread = drawing.spawn(move || drawing::draw(app, &taken, &seat))?;
    Ok(Running {
        inbox,
        thread: Some(thread),
        farewell: vec![],
    })
}

/// Where the server's frames went: when the server is gone (a daemon stopped), the interface is
/// told so and ends.
struct Gone(Sender<Input>);

impl Drop for Gone {
    fn drop(&mut self) {
        for event in [Event::Error("llx: the server is gone".into()), Event::Quit] {
            let _ = self.0.send(Input::Frame(ToClient::Event {
                session: None,
                event,
                seq: None,
            }));
        }
    }
}

/// Where this interface is: the sessions it opens work there, and their commands run with its
/// environment (a variable that is not text is left out), and the command line it was started
/// with, whose options the sessions take.
fn here(root: &std::path::Path, theme: Option<protocol::Theme>) -> Place {
    let env = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)));
    let dir = std::env::current_dir().unwrap_or_default();
    Place {
        cwd: root.display().to_string(),
        env: env.collect(),
        theme,
        args: std::env::args().skip(1).collect(),
        dir: dir.display().to_string(),
    }
}

impl Running {
    /// Until the interface ends by itself: the server said the session is over, or is gone.
    pub(crate) fn wait(mut self) {
        self.join();
    }

    fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            self.farewell = thread.join().unwrap_or_default();
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let event = Event::Quit;
        let _ = (self.inbox).send(Input::Frame(ToClient::Event {
            session: None,
            event,
            seq: None,
        }));
        self.join();
        restore();
        terminal::say(&self.farewell);
    }
}
