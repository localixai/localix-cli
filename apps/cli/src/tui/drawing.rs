//! The interface's thread: it takes what came (the server's frames, the terminal's keys), sends
//! what the person did as commands and replies, and draws, until the session is over.

use std::io;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use client::Client;
use protocol::{Command, Done, ToClient};

use super::app::{Action, App};
use super::event::{Event, Input, Out};
use super::screen::Screen;
use super::{live, style, terminal};

/// How often the live area is redrawn while something runs (the spinner's frame).
const FRAME: Duration = Duration::from_millis(80);
/// How long a command the person waits on may take.
const ANSWER: Duration = Duration::from_secs(5);

/// What the interface needs beside its state: the server, and how to end at once.
pub(super) struct Seat {
    pub client: Client,
    /// Ctrl-C twice: the process ends now.
    pub quit_now: fn() -> !,
}

pub(super) fn draw(mut app: App, inbox: &Receiver<Input>, seat: &Seat) {
    let mut screen = Screen::new(io::stdout(), style::colour_stdout());
    let mut over = false;
    let mut shown = None;
    while !over {
        let wait = if app.moving() {
            FRAME
        } else {
            Duration::from_secs(1)
        };
        let first = match inbox.recv_timeout(wait) {
            Ok(input) => Some(input),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        // Everything that came at once is drawn once. Many characters at once are a paste from
        // a terminal that does not mark pastes (a dropped file among them).
        let mut typed = 0;
        for input in first.into_iter().chain(inbox.try_iter()) {
            if let Input::Key(key) = &input
                && matches!(key.code, crossterm::event::KeyCode::Char(_))
            {
                typed += 1;
            }
            let ended = matches!(
                input,
                Input::Frame(ToClient::Event {
                    event: Event::Quit,
                    ..
                })
            );
            let action = app.input(input, Instant::now());
            over |= act(action, ended, &mut app, &mut screen, &mut shown, seat);
            flush(&mut app, &seat.client);
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

/// What the person asked for that the interface cannot do itself; whether the session is over.
fn act(
    action: Option<Action>,
    ended: bool,
    app: &mut App,
    screen: &mut Screen<io::Stdout>,
    shown: &mut Option<(
        Vec<ratatui::text::Line<'static>>,
        Option<super::screen::Cursor>,
    )>,
    seat: &Seat,
) -> bool {
    match action {
        Some(Action::Stop(session)) => stop(screen, app, seat, session),
        Some(Action::Editor) => {
            let _ = screen.clear();
            let text = terminal::editor(&app.composer.text());
            app.composer.set(text.trim_end_matches('\n'));
            *shown = None;
        }
        Some(Action::Clipboard) => match crate::attach::clipboard(&crate::attach::pasted_dir()) {
            Ok(path) => app.composer.attach(path),
            Err(why) => {
                app.event(Event::Note(why), Instant::now());
            }
        },
        Some(Action::Open(target)) => app.outbox.push(Out::Call(Command::Open { target })),
        Some(Action::Sessions) => app.outbox.push(Out::Call(Command::List)),
        // Every lane is stopped, its prompt given nothing; the server waits for them.
        Some(Action::QuitNow) => {
            app.leave();
            app.outbox.push(Out::Call(Command::QuitAll));
        }
        Some(Action::Halt(id)) => app.outbox.push(Out::Call(Command::Halt { id })),
        Some(Action::Quit) if ended => return true,
        Some(Action::Quit) => app.leave(),
        None => {}
    }
    false
}

/// What the interface has to send, in order.
fn flush(app: &mut App, client: &Client) {
    for out in app.outbox.drain(..) {
        let _ = match out {
            Out::Call(command) => client.send(command),
            Out::Reply(id, reply) => client.reply(id, reply),
        };
    }
}

/// Stop the task; asked a second time, quit at once, as Ctrl-C twice does outside the interface.
fn stop(screen: &mut Screen<io::Stdout>, app: &mut App, seat: &Seat, session: String) {
    let asked = seat.client.call(Command::Interrupt { session }, ANSWER);
    if matches!(asked, Ok(Done::Interrupted { again: true })) {
        let history = app.take_history();
        let _ = screen.frame(&history, &[], None, app.height, app.width);
        let _ = screen.clear();
        terminal::restore();
        (seat.quit_now)();
    }
}
