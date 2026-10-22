//! The terminal interface: what a person sees and types. Finished blocks go into the terminal's
//! own scrollback once (so scrolling, selecting and copying stay the terminal's); only a small
//! live area at the bottom is redrawn. Every block is a pure function from state and width to
//! styled lines, which the snapshot tests pin.
//!
//! One thread owns the terminal: it reads the keys (from a reader thread), takes the session's
//! events, and draws. The session runs on its own thread and talks to it through a [`Ui`]: it
//! reports what happens, and asks (its next entry, an approval, a question) by sending where the
//! answer goes and waiting there.

pub mod app;
pub mod asking;
pub mod cells;
pub mod composer;
pub mod event;
mod keys;
pub mod live;
pub mod markdown;
pub mod questions;
pub mod screen;
pub mod select;
pub mod style;
pub mod tool_cell;
pub mod wrap;

use std::io::{self, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use contracts::person::{Answer, Question};
use crossterm::event::{Event as Terminal, KeyEventKind};

use self::app::{Action, App};
use self::event::{Approval, Event};
use self::screen::Screen;
use crate::interrupt;

/// Bracketed paste on, and keys reported so Shift+Enter differs from Enter where the terminal can.
const ENTER: &str = "\x1b[?2004h\x1b[>1u";
const LEAVE: &str = "\x1b[<1u\x1b[?2004l\x1b[?25h";
/// How often the live area is redrawn while something runs (the spinner's frame).
const FRAME: Duration = Duration::from_millis(80);
/// How often a waiting question checks whether the task was stopped.
const POLL: Duration = Duration::from_millis(100);

/// The session's side of the interface: cheap to clone, one per thing that reports or asks.
#[derive(Clone)]
pub struct Ui {
    events: Sender<Event>,
}

/// The interface running; dropping it gives the terminal back.
pub struct Running {
    pub ui: Ui,
    thread: Option<JoinHandle<()>>,
}

/// Takes the terminal and starts drawing; `commands` are what `/` offers.
pub fn start(commands: Vec<(String, String)>) -> io::Result<Running> {
    let (width, height) = crossterm::terminal::size()?;
    crossterm::terminal::enable_raw_mode()?;
    let mut out = io::stdout();
    out.write_all(ENTER.as_bytes())?;
    out.flush()?;
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
    let (events, inbox) = channel();
    let keys = events.clone();
    std::thread::spawn(move || read_keys(&keys));
    let app = App::new(width as usize, height as usize, commands);
    let thread = std::thread::spawn(move || draw(app, &inbox));
    Ok(Running {
        ui: Ui { events },
        thread: Some(thread),
    })
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.ui.events.send(Event::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        restore();
    }
}

/// The terminal as it was: cooked, the cursor shown, nothing more reported.
fn restore() {
    let _ = crossterm::terminal::disable_raw_mode();
    let mut out = io::stdout();
    let _ = out.write_all(LEAVE.as_bytes()).and_then(|()| out.flush());
}

fn read_keys(events: &Sender<Event>) {
    loop {
        let event = match crossterm::event::read() {
            Ok(Terminal::Key(key)) if key.kind != KeyEventKind::Release => Event::Key(key),
            Ok(Terminal::Paste(text)) => Event::Paste(text),
            Ok(Terminal::Resize(width, height)) => Event::Resize(width, height),
            Ok(_) => continue,
            Err(_) => return,
        };
        if events.send(event).is_err() {
            return;
        }
    }
}

/// The interface's thread: take what comes, then draw, until the session is over.
fn draw(mut app: App, inbox: &Receiver<Event>) {
    let mut screen = Screen::new(io::stdout(), style::colour_stdout());
    let mut over = false;
    let mut shown = None;
    while !over {
        let moving = app.busy.is_some() || app.running.is_some();
        let first = match inbox.recv_timeout(if moving {
            FRAME
        } else {
            Duration::from_secs(1)
        }) {
            Ok(event) => Some(event),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        // Everything that came at once is drawn once.
        for event in first.into_iter().chain(inbox.try_iter()) {
            let quit = matches!(event, Event::Quit);
            match app.event(event, Instant::now()) {
                Some(Action::Stop) => stop(&mut screen, &mut app),
                Some(Action::Quit) if quit => over = true,
                Some(Action::Quit) => app.leave(),
                None => {}
            }
        }
        let now = Instant::now();
        let history = app.take_history();
        let drawn = match over {
            true => (vec![], None),
            false => live::render(&app, now),
        };
        // An idle tick or a key that changed nothing draws nothing.
        if history.is_empty() && shown.as_ref() == Some(&drawn) {
            continue;
        }
        let _ = screen.frame(&history, &drawn.0, drawn.1, app.height, app.width);
        shown = Some(drawn);
    }
    let _ = screen.clear();
}

/// Stop the task; asked a second time, quit at once, as Ctrl-C twice does outside the interface.
fn stop(screen: &mut Screen<io::Stdout>, app: &mut App) {
    if interrupt::PROCESS.stop() {
        let history = app.take_history();
        let _ = screen.frame(&history, &[], None, app.height, app.width);
        let _ = screen.clear();
        restore();
        interrupt::quit_now();
    }
}

impl Ui {
    pub fn send(&self, event: Event) {
        let _ = self.events.send(event);
    }

    /// The person's next entry; `None` when they leave.
    pub fn next_entry(&self) -> Option<String> {
        let (reply, answer) = channel();
        self.send(Event::Ready(reply));
        answer.recv().ok().flatten()
    }

    /// Whether an effect may happen; a stopped task closes the question with a no.
    pub fn approve(&self, tool: &str, summary: &str, stopped: &dyn Fn() -> bool) -> Approval {
        let (reply, answer) = channel();
        self.send(Event::Approve {
            tool: tool.into(),
            summary: summary.into(),
            reply,
        });
        self.wait(&answer, stopped).unwrap_or(Approval::No(None))
    }

    /// One of the model's questions; `None` when it was skipped or the task stopped.
    pub fn ask(
        &self,
        question: &Question,
        at: usize,
        of: usize,
        stopped: &dyn Fn() -> bool,
    ) -> Option<Answer> {
        let (reply, answer) = channel();
        self.send(Event::Ask {
            question: question.clone(),
            at,
            of,
            reply,
        });
        self.wait(&answer, stopped).flatten()
    }

    /// One of `options`, or `None`.
    pub fn choose(&self, title: &str, options: &[&str]) -> Option<usize> {
        let (reply, answer) = channel();
        self.send(Event::Choose {
            title: title.into(),
            options: options.iter().map(|option| option.to_string()).collect(),
            reply,
        });
        answer.recv().ok().flatten()
    }

    fn wait<T>(&self, answer: &Receiver<T>, stopped: &dyn Fn() -> bool) -> Option<T> {
        loop {
            match answer.recv_timeout(POLL) {
                Ok(value) => return Some(value),
                Err(RecvTimeoutError::Disconnected) => return None,
                Err(RecvTimeoutError::Timeout) if stopped() => {
                    self.send(Event::Dismiss);
                    return None;
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }
}
