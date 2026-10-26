//! Test-only: a session at the desk, its lanes thinking with gated models, and a person at a real
//! [`App`], attached to a real server through the protocol's frames, acting as the interface's
//! thread does.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use contracts::chat::{Completion, Message, ToolCall};
use crossterm::event::KeyCode;
use protocol::{Command, Event, ToClient, ToServer};
use serde_json::Value;

use super::{Seat, Shared, Work, run};
use crate::compose::{Models, lane_with};
use crate::server::serve::{Info, serve};
use crate::server::{Peer, Ui};
use crate::testing::{Gated, NoDecider, Scratch, setup};
use crate::tui::app::{Action, App};
use crate::tui::event::{Input, Out};
use crate::tui::test_support::{key, typed};
use crate::tui::views::AgentState;

/// The person at the interface: a real [`App`] fed what the server sends, its commands and
/// replies sent back, as the interface's thread does.
pub(crate) struct Person {
    pub app: App,
    pub frames: Receiver<ToClient>,
    pub to_server: Sender<ToServer>,
    pub over: bool,
}

impl Person {
    /// Take what the server sends until `done` holds.
    pub(crate) fn until(&mut self, what: &str, done: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(&self.app) {
            if Instant::now() >= deadline {
                let views: Vec<_> = (self.app.views.sessions.iter())
                    .map(|v| (v.id.clone(), v.state.clone(), v.busy.is_some()))
                    .collect();
                panic!("waited too long for {what}: {views:?}");
            }
            self.take();
        }
    }

    /// One frame, if one comes soon.
    fn take(&mut self) {
        if let Ok(frame) = self.frames.recv_timeout(Duration::from_millis(20)) {
            let ended = matches!(
                &frame,
                ToClient::Event {
                    event: Event::Quit,
                    ..
                }
            );
            let action = self.app.input(Input::Frame(frame), Instant::now());
            self.act(action, ended);
        }
    }

    /// Take what the server sends for `millis`.
    pub(crate) fn wait(&mut self, millis: u64) {
        let until = Instant::now() + Duration::from_millis(millis);
        self.until("time to pass", |_| Instant::now() >= until);
    }

    /// Take what the server sends until the interface is told to end.
    pub(crate) fn until_over(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !self.over {
            assert!(Instant::now() < deadline, "waited too long for the end");
            self.take();
        }
    }

    pub(crate) fn act(&mut self, action: Option<Action>, ended: bool) {
        let call = match action {
            Some(Action::Open(target)) => Some(Command::Open { target }),
            Some(Action::Stop(session)) => Some(Command::Interrupt { session }),
            Some(Action::Sessions) => Some(Command::List),
            Some(Action::Halt(id)) => Some(Command::Halt { id }),
            Some(Action::QuitNow) => {
                self.app.leave();
                Some(Command::QuitAll)
            }
            Some(Action::Quit) if ended => {
                self.over = true;
                None
            }
            Some(Action::Quit) => {
                self.app.leave();
                None
            }
            _ => None,
        };
        self.app.outbox.extend(call.map(Out::Call));
        self.flush();
    }

    /// What the interface has to send, in order.
    pub(crate) fn flush(&mut self) {
        for (id, out) in self.app.outbox.drain(..).enumerate() {
            let frame = match out {
                Out::Call(command) => ToServer::Call {
                    id: id as u64 + 1_000,
                    command,
                },
                Out::Reply(id, reply) => ToServer::Reply { id, reply },
            };
            let _ = self.to_server.send(frame);
        }
    }

    /// Send `command` as the interface would.
    pub(crate) fn command(&mut self, command: Command) {
        self.app.outbox.push(Out::Call(command));
        self.flush();
    }

    /// Type `text` and send it.
    pub(crate) fn enter(&mut self, text: &str) {
        typed(&mut self.app, text, Instant::now());
        let action = self.app.input(key(KeyCode::Enter), Instant::now());
        self.act(action, false);
    }

    /// The lanes' ids, the first opened first.
    pub(crate) fn lanes(&self) -> Vec<String> {
        let sessions = &self.app.views.sessions;
        let lanes = sessions.iter().filter(|view| view.lane.is_some());
        lanes.map(|view| view.id.clone()).collect()
    }

    /// The first lane's id, once the interface knows it.
    pub(crate) fn first(&mut self) -> String {
        self.until("the first lane", |app| !app.views.sessions[0].id.is_empty());
        self.app.views.sessions[0].id.clone()
    }

    /// `/new`, and the new lane shown, waiting for its first entry; its id.
    pub(crate) fn new_lane(&mut self) -> String {
        let before = self.app.views.shown.clone();
        self.enter("/new");
        self.until("the new lane", |app| {
            let shown = app.shown();
            let waits = shown.lane.as_ref().is_some_and(|l| l.ready);
            shown.id != before && waits
        });
        self.app.views.shown.clone()
    }
}

pub(crate) fn state(app: &App, id: &str) -> AgentState {
    let at = app.views.find(id).expect("a known session");
    app.views.sessions[at].state.clone()
}

pub(crate) fn ended(app: &App, id: &str) -> bool {
    matches!(state(app, id), AgentState::Ended(_))
}

pub(crate) fn call(name: &str, arguments: Value) -> Completion {
    let call = ToolCall {
        id: format!("c-{name}"),
        name: name.into(),
        arguments,
    };
    Completion {
        calls: vec![call],
        ..Completion::default()
    }
}

/// A session at the desk, its first task `task`, its lanes thinking with `chats` in the order
/// they open (a lane past them panics as it starts); `person` acts, then leaves, and the desk
/// must end. The session store, for what was recorded.
pub(crate) fn at_desk(
    chats: &[&Gated],
    task: Option<&str>,
    person: impl FnOnce(&mut Person),
) -> Scratch {
    at_desk_with(chats, task, &|| panic!("the desk quit hard"), person)
}

/// [`at_desk`], with `quit` for what ends the process when lanes do not stop in time.
pub(crate) fn at_desk_with(
    chats: &[&Gated],
    task: Option<&str>,
    quit: &(dyn Fn() + Sync),
    person: impl FnOnce(&mut Person),
) -> Scratch {
    connected(chats, task, quit, |mut at, _| {
        person(&mut at);
        at.app.leave();
        at.flush();
        at.until_over();
    })
}

/// A session at the desk as [`at_desk_with`] makes it, `person` given the connection to end it
/// as they choose, and the server's side of it; the desk must end within 30 s after.
#[expect(
    clippy::disallowed_methods,
    reason = "the desk's end is looked at again for a bounded time: with the person gone, no \
              frame says it"
)]
pub(crate) fn connected(
    chats: &[&Gated],
    task: Option<&str>,
    quit: &(dyn Fn() + Sync),
    person: impl FnOnce(Person, &Arc<Peer>),
) -> Scratch {
    let scratch = Scratch::new();
    let (shared, seat) = setup(&scratch, crate::modes::built("bypass"), true).split(None);
    #[expect(
        clippy::disallowed_methods,
        reason = "the in-process transport and the desk's requests, as the binary makes them"
    )]
    let ((to_server, incoming), (out, frames), (to_desk, asked)) =
        (channel(), channel(), channel());
    let peer = Peer::new(out);
    let ui = Ui::new(Arc::clone(&peer));
    let seat = Seat {
        ui: Some(ui.clone()),
        ..seat
    };
    let next = AtomicUsize::new(0);
    let start = |shared: &Shared, seat: Seat, work: Work| {
        let n = next.fetch_add(1, Ordering::SeqCst);
        let chat = *chats.get(n).expect("a lane the test meant to panic");
        let models = Models {
            chat,
            decider: &NoDecider,
            live: None,
        };
        lane_with(shared, seat, models, work)
    };
    std::thread::scope(|scope| {
        let task = task.map(str::to_string);
        let desk = scope.spawn(|| run(&shared, &ui, asked, seat, task, &start, quit));
        let info = Info {
            max_lanes: super::MAX_LANES,
            shells: Arc::new(Vec::new),
            commands: vec![],
            modes: vec![],
            background: false,
            host: None,
        };
        let served = Arc::clone(&peer);
        scope.spawn(move || serve(&incoming, &served, &to_desk, &info));
        let mut at = Person {
            app: App::new(80, 24, vec![]),
            frames,
            to_server,
            over: false,
        };
        at.app.max_lanes = super::MAX_LANES;
        person(at, &peer);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !desk.is_finished() {
            assert!(
                Instant::now() < deadline,
                "waited too long for the desk to end"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(desk.join().unwrap(), crate::exit::OK);
    });
    scratch
}

/// The text of the last call's result in a conversation.
pub(crate) fn last_result(conversation: &[Message]) -> String {
    let mut results = conversation
        .iter()
        .rev()
        .filter_map(|message| match message {
            Message::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        });
    results.next().unwrap_or_default()
}
