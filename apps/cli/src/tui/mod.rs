//! The terminal interface: what a person sees and types. Finished blocks go into the terminal's
//! own scrollback once (so scrolling, selecting and copying stay the terminal's); only a small
//! live area at the bottom is redrawn. Every block is a pure function from state and width to
//! styled lines, which the snapshot tests pin.
//!
//! One thread owns the terminal: it reads the keys (from a reader thread), takes the session's
//! events, and draws. The session runs on its own thread and talks to it through a [`Ui`]: it
//! reports what happens, and asks (its next entry, an approval, a question) by sending where the
//! answer goes and waiting there.

pub(crate) mod agents;
pub(crate) mod app;
pub(crate) mod asking;
pub(crate) mod cells;
pub(crate) mod composer;
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
pub(crate) mod steer;
pub(crate) mod style;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod tool_cell;
pub(crate) mod tree;
mod ui;
pub(crate) mod views;
pub(crate) mod wrap;

use std::io::{self, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossterm::event::{Event as Terminal, KeyEventKind};

use self::app::{Action, App};
use self::event::Event;
use self::screen::Screen;
pub(crate) use self::ui::Ui;
use crate::interrupt;

/// Bracketed paste on, and keys reported so Shift+Enter differs from Enter where the terminal can.
const ENTER: &str = "\x1b[?2004h\x1b[>1u";
const LEAVE: &str = "\x1b[<1u\x1b[?2004l\x1b[?25h";
/// How often the live area is redrawn while something runs (the spinner's frame).
const FRAME: Duration = Duration::from_millis(80);

/// The interface running; dropping it gives the terminal back.
pub(crate) struct Running {
    pub ui: Ui,
    thread: Option<JoinHandle<()>>,
}

/// Takes the terminal and starts drawing; `commands` are what `/` offers. With it, what the
/// person asks of the desk (a new lane), for whoever runs the lanes.
pub(crate) fn start(
    commands: Vec<(String, String)>,
    root: &std::path::Path,
) -> io::Result<(Running, Receiver<crate::desk::Request>)> {
    let (width, height) = crossterm::terminal::size()?;
    crossterm::terminal::enable_raw_mode()?;
    let mut out = io::stdout();
    // Nowhere to draw (stdout's reader gone): the terminal goes back as it was.
    if let Err(error) = out.write_all(ENTER.as_bytes()).and_then(|()| out.flush()) {
        restore();
        return Err(error);
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
    let (events, inbox) = channel();
    let keys = events.clone();
    std::thread::spawn(move || read_keys(&keys));
    let mut app = App::new(width as usize, height as usize, commands);
    app.files = mention::Files::new(Some(root.to_path_buf()));
    app.shells = tools::shell::running_anywhere;
    let (asks, requests) = channel();
    let thread = std::thread::spawn(move || draw(app, &inbox, &asks));
    let running = Running {
        ui: Ui::new(events),
        thread: Some(thread),
    };
    Ok((running, requests))
}

impl Drop for Running {
    fn drop(&mut self) {
        self.ui.send(Event::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        restore();
    }
}

/// The terminal as it was: cooked, the cursor shown, nothing more reported.
pub(crate) fn restore() {
    let _ = crossterm::terminal::disable_raw_mode();
    let mut out = io::stdout();
    let _ = out.write_all(LEAVE.as_bytes()).and_then(|()| out.flush());
}

/// The editor has the terminal: the keys are its, not the interface's.
static PAUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn read_keys(events: &Sender<Event>) {
    use std::sync::atomic::Ordering;
    loop {
        if PAUSED.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }
        match crossterm::event::poll(Duration::from_millis(50)) {
            Ok(true) if !PAUSED.load(Ordering::SeqCst) => {}
            Ok(_) => continue,
            Err(_) => return,
        }
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
fn draw(mut app: App, inbox: &Receiver<Event>, requests: &Sender<crate::desk::Request>) {
    let mut screen = Screen::new(io::stdout(), style::colour_stdout());
    let mut over = false;
    let mut shown = None;
    while !over {
        let moving = app.moving();
        let first = match inbox.recv_timeout(if moving {
            FRAME
        } else {
            Duration::from_secs(1)
        }) {
            Ok(event) => Some(event),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        // Everything that came at once is drawn once. Many characters at once are a paste from
        // a terminal that does not mark pastes (a dropped file among them).
        let mut typed = 0;
        for event in first.into_iter().chain(inbox.try_iter()) {
            if let Event::Key(key) = &event
                && matches!(key.code, crossterm::event::KeyCode::Char(_))
            {
                typed += 1;
            }
            let quit = matches!(event, Event::Quit);
            match app.event(event, Instant::now()) {
                Some(Action::Stop(id)) => stop(&mut screen, &mut app, &id),
                Some(Action::Editor) => {
                    let _ = screen.clear();
                    let text = editor(&app.composer.text());
                    app.composer.set(text.trim_end_matches('\n'));
                    shown = None;
                }
                Some(Action::Clipboard) => {
                    match crate::attach::clipboard(&crate::attach::pasted_dir()) {
                        Ok(path) => app.composer.attach(path),
                        Err(why) => {
                            app.event(Event::Note(why), Instant::now());
                        }
                    }
                }
                Some(Action::Open(target)) => {
                    let _ = requests.send(crate::desk::Request::Open(target));
                }
                Some(Action::Sessions) => {
                    let _ = requests.send(crate::desk::Request::List);
                }
                // Every lane is stopped, its prompt given nothing; the desk waits for them.
                Some(Action::QuitNow) => {
                    app.leave();
                    let _ = requests.send(crate::desk::Request::Quit);
                }
                Some(Action::Halt(id)) => {
                    interrupt::stop(&id);
                }
                Some(Action::Quit) if quit => over = true,
                Some(Action::Quit) => app.leave(),
                None => {}
            }
        }
        if typed >= 4 && app.questions.is_empty() {
            app.adopt_paths();
        }
        let now = Instant::now();
        if app.take_replay() {
            let _ = screen.reset();
        }
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

/// The composer's `text` written in `$VISUAL` or `$EDITOR` (else `vi`), the terminal handed over
/// meanwhile; the text as saved, or as it was when the editor could not run.
fn editor(text: &str) -> String {
    let path = std::env::temp_dir().join(format!("llx-prompt-{}.md", std::process::id()));
    if std::fs::write(&path, text).is_err() {
        return text.to_string();
    }
    PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
    // The reader may be between a poll and its read: give it the time to see the pause.
    std::thread::sleep(Duration::from_millis(60));
    restore();
    let command = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    let ran = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{command} \"$0\""))
        .arg(&path)
        .status();
    let _ = crossterm::terminal::enable_raw_mode();
    let mut out = io::stdout();
    let _ = out.write_all(ENTER.as_bytes()).and_then(|()| out.flush());
    PAUSED.store(false, std::sync::atomic::Ordering::SeqCst);
    let edited = ran
        .ok()
        .filter(|status| status.success())
        .and_then(|_| std::fs::read_to_string(&path).ok());
    let _ = std::fs::remove_file(&path);
    edited.unwrap_or_else(|| text.to_string())
}

/// Stop the task; asked a second time, quit at once, as Ctrl-C twice does outside the interface.
fn stop(screen: &mut Screen<io::Stdout>, app: &mut App, id: &str) {
    if interrupt::stop(id) {
        let history = app.take_history();
        let _ = screen.frame(&history, &[], None, app.height, app.width);
        let _ = screen.clear();
        restore();
        interrupt::quit_now();
    }
}
