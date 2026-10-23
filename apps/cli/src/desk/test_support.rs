//! Test-only: a session at the desk, its lanes thinking with gated models, and a person at a real
//! [`App`] fed what the lanes send, acting as the interface's thread does.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use contracts::chat::{Completion, Message, ToolCall};
use crossterm::event::KeyCode;
use serde_json::Value;

use super::{Request, Seat, Shared, Work, run};
use crate::compose::{Models, lane_with};
use crate::mode::Mode;
use crate::testing::{Gated, NoDecider, Scratch, setup};
use crate::tui::Ui;
use crate::tui::app::{Action, App};
use crate::tui::event::Event;
use crate::tui::test_support::{key, typed};
use crate::tui::views::AgentState;

/// The person at the interface: a real [`App`] fed what the lanes send, and the requests its
/// actions make, as the interface's thread does.
pub(crate) struct Person {
    pub app: App,
    pub events: Receiver<Event>,
    pub requests: Sender<Request>,
    pub over: bool,
}

impl Person {
    /// Take what the lanes send until `done` holds.
    pub(crate) fn until(&mut self, what: &str, done: impl Fn(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(&self.app) {
            if Instant::now() >= deadline {
                let views: Vec<_> = (self.app.views.sessions.iter())
                    .map(|v| (v.id.clone(), v.state.clone(), v.busy.is_some()))
                    .collect();
                panic!("waited too long for {what}: {views:?}");
            }
            if let Ok(event) = self.events.recv_timeout(Duration::from_millis(20)) {
                let quit = matches!(event, Event::Quit);
                let action = self.app.event(event, Instant::now());
                self.act(action, quit);
            }
        }
    }

    /// Take what the lanes send for `millis`.
    pub(crate) fn wait(&mut self, millis: u64) {
        let until = Instant::now() + Duration::from_millis(millis);
        self.until("time to pass", |_| Instant::now() >= until);
    }

    /// Take what the lanes send until the interface is told to end.
    pub(crate) fn until_over(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.over {
            assert!(Instant::now() < deadline, "waited too long for the end");
            if let Ok(event) = self.events.recv_timeout(Duration::from_millis(20)) {
                let quit = matches!(event, Event::Quit);
                let action = self.app.event(event, Instant::now());
                self.act(action, quit);
            }
        }
    }

    pub(crate) fn act(&mut self, action: Option<Action>, quit: bool) {
        match action {
            Some(Action::Open(target)) => self.requests.send(Request::Open(target)).unwrap(),
            Some(Action::Stop(id)) => {
                crate::interrupt::stop(&id);
            }
            Some(Action::Sessions) => self.requests.send(Request::List).unwrap(),
            Some(Action::Halt(id)) => {
                crate::interrupt::stop(&id);
            }
            Some(Action::QuitNow) => {
                self.app.leave();
                self.requests.send(Request::Quit).unwrap();
            }
            Some(Action::Quit) if quit => self.over = true,
            Some(Action::Quit) => self.app.leave(),
            _ => {}
        }
    }

    /// Type `text` and send it.
    pub(crate) fn enter(&mut self, text: &str) {
        typed(&mut self.app, text, Instant::now());
        let action = self.app.event(key(KeyCode::Enter), Instant::now());
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
            let waits = shown.lane.as_ref().is_some_and(|l| l.waiting.is_some());
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
    let scratch = Scratch::new();
    let (shared, seat) = setup(&scratch, Mode::Bypass, true).split(None);
    let (events, inbox) = channel();
    let ui = Ui::new(events);
    let (requests, asked) = channel();
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
        let mut at = Person {
            app: App::new(80, 24, vec![]),
            events: inbox,
            requests,
            over: false,
        };
        person(&mut at);
        at.app.leave();
        at.until("the desk to end", |_| desk.is_finished());
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
