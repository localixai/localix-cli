//! The session's interface as state: what has finished (and is yet to be printed), what runs now,
//! what the person is typing, and the question open. It takes events and keys, says what the
//! person wants done (send an entry, stop the task, quit), and is drawn by [`super::live`].
//! Nothing here touches the terminal, so the tests drive it as the person would.

use std::collections::VecDeque;
use std::time::Instant;

use protocol::{Command, Input as Sent, ToClient};
use ratatui::text::Line;

use super::agents::{LaneState, Views};
use super::attach;
use super::composer::Composer;
use super::event::{Event, Input, JobRow, Out, Typed};
use super::mention::Files;
use super::questions;
use super::queue::Asked;
use crate::tui::printable::printable;

/// What the person asked for that the interface cannot do itself.
#[derive(Debug, PartialEq)]
pub(crate) enum Action {
    /// Stop the task of the lane named.
    Stop(String),
    /// Leave the session now.
    Quit,
    /// Attach the clipboard's picture.
    Clipboard,
    /// Put a session's text in the clipboard (`/copy`).
    Copy(String),
    /// Write the composer's text in `$EDITOR`.
    Editor,
    /// Open a lane (`/new`), whatever the lane shown is doing.
    Open(protocol::Target),
    /// The list of sessions opened: the saved ones are asked for.
    Sessions,
    /// Stop the task of the lane named, and never quit for it (`x` on its row).
    Halt(String),
    /// Quit although work runs: every lane is stopped first ("Quit anyway").
    QuitNow,
}

/// What Shift+Tab sends: the session reads it as the plan-mode switch.
pub(crate) use protocol::SHIFT_TAB;

/// A question open, by the id its reply goes under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Open {
    Approval(u64),
    Question(u64),
    Choice(u64),
    Adjustable(u64),
    Line(u64),
    /// Whether to quit while work runs: the interface's own, nobody waits for it.
    Quit,
}

impl Open {
    /// The id of the server's question; `None` for the interface's own.
    pub(crate) fn id(self) -> Option<u64> {
        match self {
            Self::Approval(id)
            | Self::Question(id)
            | Self::Choice(id)
            | Self::Adjustable(id)
            | Self::Line(id) => Some(id),
            Self::Quit => None,
        }
    }
}

pub(crate) struct App {
    pub width: usize,
    pub height: usize,
    /// Finished blocks not printed yet.
    pub(super) pending: Vec<Vec<Line<'static>>>,
    /// The main session and its agents, each a transcript, and the one shown.
    pub views: Views,
    pub composer: Composer,
    /// The commands `/` offers: each name and what it does.
    pub commands: Vec<(String, String)>,
    /// The command marked among those offered.
    pub offered: usize,
    /// The workspace's files, for `@`.
    pub files: Files,
    /// Where pictures dropped in are kept.
    pub pasted: std::path::PathBuf,
    /// The question open first, then those waiting their turn.
    pub questions: VecDeque<Asked>,
    /// Until when the question open ignores keys: what was typed ahead for the composer as it
    /// opened must not answer it.
    pub(super) settling: Option<Instant>,
    /// Ctrl-C at an empty prompt: a second one quits, until this moment.
    pub quitting: Option<Instant>,
    /// That Ctrl-C cleared a draft, which ↑ brings back.
    pub(super) discarded: bool,
    /// Esc at an empty prompt: a second one rewinds, until this moment.
    pub(super) escaped: Option<Instant>,
    /// The person left: the sessions' next waits end with nothing.
    pub(super) left: bool,
    /// The list of sessions, while it is open.
    pub(super) listing: Option<super::sessions::SessionList>,
    /// This directory's saved sessions, as the desk last listed them.
    pub(super) saved: Vec<(String, String, String)>,
    /// A lane closed to make room, said in the next lane that opens.
    pub(super) closed: Option<String>,
    /// How many shells run, in every session (asked of the server): quitting stops them.
    pub shells: Box<dyn Fn() -> usize + Send>,
    /// The modes `/mode` offers, each with what it does, as the server says them.
    pub modes: Vec<(String, String)>,
    /// Lanes the server keeps open at most.
    pub max_lanes: usize,
    /// What the interface sends next: commands and replies, in order.
    pub outbox: Vec<Out>,
}
/// How long a first Ctrl-C at the prompt waits for a second.
pub(super) const QUIT_WINDOW_MS: u128 = 2000;

impl App {
    pub(crate) fn new(width: usize, height: usize, commands: Vec<(String, String)>) -> Self {
        Self {
            width,
            height,
            pending: vec![],
            views: Views::default(),
            composer: Composer::default(),
            commands,
            offered: 0,
            files: Files::new(None),
            pasted: attach::pasted_dir(),
            questions: VecDeque::new(),
            settling: None,
            quitting: None,
            discarded: false,
            escaped: None,
            left: false,
            listing: None,
            saved: vec![],
            closed: None,
            shells: Box::new(|| 0),
            modes: vec![],
            max_lanes: 8,
            outbox: vec![],
        }
    }

    /// The finished blocks to print, with a blank row between them; they are printed once.
    pub(crate) fn take_history(&mut self) -> Vec<Line<'static>> {
        let mut out = vec![];
        for block in self.pending.drain(..) {
            out.extend(block);
            out.push(Line::default());
        }
        out
    }

    /// The lane of the session shown: what the person types goes there.
    #[allow(
        clippy::expect_used,
        reason = "`home` is a lane: an agent's view is its parent's, and the first view is a lane"
    )]
    pub(crate) fn lane(&self) -> &LaneState {
        let lane = self.views.sessions[self.views.home()].lane.as_deref();
        lane.expect("a lane")
    }

    #[allow(
        clippy::expect_used,
        reason = "`home` is a lane: an agent's view is its parent's, and the first view is a lane"
    )]
    pub(super) fn lane_mut(&mut self) -> &mut LaneState {
        let home = self.views.home();
        let lane = self.views.sessions[home].lane.as_deref_mut();
        lane.expect("a lane")
    }

    /// When the task of the lane shown started, while it runs.
    pub(crate) fn busy(&self) -> Option<Instant> {
        self.views.sessions[self.views.home()].busy
    }

    /// What the lane shown is known by: an agent's view names it.
    pub(crate) fn home_title(&self) -> String {
        printable(self.views.sessions[self.views.home()].title())
    }

    /// The id of the lane shown: its stop is known by it.
    pub(crate) fn lane_id(&self) -> &str {
        &self.views.sessions[self.views.home()].id
    }

    /// What the interface's thread took: a frame from the server, or the terminal's.
    pub(crate) fn input(&mut self, input: Input, now: Instant) -> Option<Action> {
        match input {
            Input::Frame(frame) => self.frame(frame, now),
            Input::Key(key) => self.key(key, now),
            Input::Paste(text) => {
                self.paste(&text);
                None
            }
            Input::Resize(width, height) => {
                self.width = width as usize;
                self.height = height as usize;
                None
            }
        }
    }

    /// A frame from the server: an event of a session or of the server, a question, a
    /// question withdrawn.
    pub(crate) fn frame(&mut self, frame: ToClient, now: Instant) -> Option<Action> {
        match frame {
            ToClient::Event {
                session: Some(id),
                event,
            } => self.event_in(&id, event, now),
            ToClient::Event {
                session: None,
                event,
            } => self.event(event, now),
            ToClient::Ask { id, session, asked } => {
                self.asked(id, &session, asked, now);
                None
            }
            ToClient::Resolved { id } => {
                self.resolved(id, now);
                None
            }
            // What a command came to is the client's.
            ToClient::Done { .. } => None,
        }
    }

    /// What happened in session `id` (a lane or an agent); one no longer shown is let go.
    pub(crate) fn event_in(&mut self, id: &str, event: Event, now: Instant) -> Option<Action> {
        let at = self.views.find(id)?;
        self.on(at, event, now)
    }

    /// What happened in the server as a whole; anything else is the lane shown's.
    pub(crate) fn event(&mut self, event: Event, now: Instant) -> Option<Action> {
        match event {
            Event::LaneStarted { id, name } => {
                self.views.lane_started(id.clone(), name);
                if let (Some(said), Some(at)) = (self.closed.take(), self.views.find(&id)) {
                    self.push(at, super::cells::note(&printable(&said), self.width));
                }
                None
            }
            Event::Show(id) => {
                self.show(&id);
                None
            }
            Event::Saved(rows) => {
                self.saved(rows.into_iter().map(|s| (s.id, s.title, s.meta)).collect());
                None
            }
            Event::Quit => Some(Action::Quit),
            event => self.on(self.views.home(), event, now),
        }
    }

    /// The server asks the person, for `session`'s task: an approval and the model's questions
    /// wait their turn; a choice of the lane's own waits until it is shown.
    pub(crate) fn asked(&mut self, id: u64, session: &str, asked: protocol::Asked, now: Instant) {
        let at = self
            .views
            .find(session)
            .unwrap_or_else(|| self.views.home());
        let lane = self.lane_at(at);
        let label = self.label(lane);
        match asked {
            protocol::Asked::Approve { tool, summary } => {
                let select = questions::approval(&tool, &summary, label.as_deref());
                self.ask(lane, select, Open::Approval(id), true, now);
            }
            protocol::Asked::Question { question, at, of } => {
                let select = questions::question(&question, at, of, label.as_deref());
                self.ask(lane, select, Open::Question(id), false, now);
            }
            protocol::Asked::Choose {
                title,
                options,
                marked,
            } => {
                let select = questions::choice(&title, &options, marked);
                self.ask_own(lane, select, Open::Choice(id), now);
            }
            protocol::Asked::ChooseAdjustable {
                title,
                options,
                marked,
                adjustments,
            } => {
                let select = questions::choice(&title, &options, marked).adjustable(adjustments);
                self.ask_own(lane, select, Open::Adjustable(id), now);
            }
            protocol::Asked::Line { prompt, hidden } => {
                let select = questions::line(&prompt, hidden);
                self.ask_own(lane, select, Open::Line(id), now);
            }
        }
    }

    /// What is the interface's as a whole: the jobs, the composer's words.
    /// `at` is the session the event came from.
    pub(super) fn global(&mut self, at: usize, event: Event) -> Option<Action> {
        let lane = self.lane_at(at);
        match event {
            // Each session's jobs, in the order of the sessions.
            Event::Jobs(rows) => {
                let id = self.views.sessions[lane].id.clone();
                self.views.jobs.retain(|row| row.lane != id);
                self.views.jobs.extend(rows.into_iter().map(|row| JobRow {
                    lane: id.clone(),
                    ..row
                }));
                let mut jobs = std::mem::take(&mut self.views.jobs);
                jobs.sort_by_key(|row| self.views.find(&row.lane));
                self.views.jobs = jobs;
                // A row that went takes the list's keys to the last one left.
                let last = self.last_row();
                self.views.focus = self.views.focus.map(|row| row.min(last));
            }
            Event::Prefill(text) => self.composer.set(&text),
            Event::Copy(text) => return Some(Action::Copy(text)),
            Event::Quit => return Some(Action::Quit),
            // A session's own events reach it through [`App::on`].
            _ => {}
        }
        None
    }

    /// What the terminal pasted: into the question open, else the composer. A dropped file
    /// arrives as a path; other clipboard text is shown as [Paste].
    fn paste(&mut self, text: &str) {
        if let Some(asked) = self.questions.front_mut() {
            return asked.select.paste(text);
        }
        if Composer::is_path_list(text, &attach::resolve) {
            self.composer.paste(text);
            self.adopt_paths();
        } else {
            self.composer.paste_marked(text);
        }
    }

    /// An entry for the lane shown: the server gives it to the lane at its prompt, to the task
    /// running (words), or keeps it for after.
    pub(super) fn send(&mut self, entry: Typed) {
        // What the person types is for the lane: it is shown again to see it go there.
        let lane = self.lane_id().to_string();
        self.show(&lane);
        self.outbox.push(Out::Call(Command::Input(Sent {
            session: lane,
            text: entry.text,
            images: entry.images,
            later: false,
        })));
    }

    /// Each path in the composer that names a file becomes its mark; a picture is kept a copy of.
    pub(crate) fn adopt_paths(&mut self) {
        let dir = self.pasted.clone();
        let keep = |path: &std::path::Path| attach::keep(path, &dir);
        self.composer.adopt(&attach::resolve, &keep);
    }

    /// The person is leaving: what waits for them ends with nothing.
    pub(crate) fn leave(&mut self) {
        self.left = true;
        self.outbox.push(Out::Call(Command::Leave));
        self.close_questions();
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
