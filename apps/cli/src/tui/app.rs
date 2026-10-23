//! The session's interface as state: what has finished (and is yet to be printed), what runs now,
//! what the person is typing, and the question open. It takes events and keys, says what the
//! person wants done (send an entry, stop the task, quit), and is drawn by [`super::live`].
//! Nothing here touches the terminal, so the tests drive it as the person would.

use std::collections::VecDeque;
use std::sync::mpsc::SyncSender;
use std::time::Instant;

use protocol::Answer;
use ratatui::text::Line;

use super::agents::{LaneState, Views};
use super::composer::Composer;
use super::event::{Approval, Event, JobRow, Typed};
use super::mention::Files;
use super::questions;
use super::queue::Asked;
use super::steer::steerable;
use crate::attach;
use crate::printable::printable;

/// What the person asked for that the interface cannot do itself.
#[derive(Debug, PartialEq)]
pub(crate) enum Action {
    /// Stop the task of the lane named.
    Stop(String),
    /// Leave the session now.
    Quit,
    /// Attach the clipboard's picture.
    Clipboard,
    /// Write the composer's text in `$EDITOR`.
    Editor,
    /// Open a lane (`/new`), whatever the lane shown is doing.
    Open(crate::desk::Target),
    /// The list of sessions opened: the saved ones are asked for.
    Sessions,
    /// Stop the task of the lane named, and never quit for it (`x` on its row).
    Halt(String),
    /// Quit although work runs: every lane is stopped first ("Quit anyway").
    QuitNow,
}

/// What Shift+Tab sends: the session reads it as the plan-mode switch.
pub(crate) use protocol::SHIFT_TAB;

/// A question open, and where its answer goes.
pub(crate) enum Open {
    Approval(SyncSender<Approval>),
    Question(SyncSender<Option<Answer>>),
    Choice(SyncSender<Option<usize>>),
    Adjustable(SyncSender<Option<(usize, usize)>>),
    Line(SyncSender<Option<String>>),
    /// Whether to quit while work runs: the interface's own, nobody waits for it.
    Quit,
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
    /// How many shells run, in every session: quitting stops them.
    pub shells: fn() -> usize,
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
            shells: || 0,
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

    pub(crate) fn event(&mut self, event: Event, now: Instant) -> Option<Action> {
        match event {
            Event::In(id, inner) => match self.views.find(&id) {
                Some(at) => self.on(at, *inner, now),
                None => None,
            },
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
            // What names no session is the lane's shown.
            event => self.on(self.views.home(), event, now),
        }
    }

    /// What is the interface's as a whole: the questions, the jobs, the terminal's.
    /// `at` is the session the event came from (the lane shown for the terminal's own).
    pub(super) fn global(&mut self, at: usize, event: Event, now: Instant) -> Option<Action> {
        let lane = self.lane_at(at);
        match event {
            Event::Approve {
                tool,
                summary,
                reply,
            } => {
                let label = self.label(lane);
                let select = questions::approval(&tool, &summary, label.as_deref());
                self.ask(lane, select, Open::Approval(reply), true, now);
            }
            Event::Ask {
                question,
                at,
                of,
                reply,
            } => {
                let label = self.label(lane);
                let select = questions::question(&question, at, of, label.as_deref());
                self.ask(lane, select, Open::Question(reply), false, now);
            }
            Event::Choose {
                title,
                options,
                marked,
                reply,
            } => {
                let select = questions::choice(&title, &options, marked);
                self.ask_own(lane, select, Open::Choice(reply), now);
            }
            Event::ChooseAdjustable {
                title,
                options,
                marked,
                adjustments,
                reply,
            } => {
                let select = questions::choice(&title, &options, marked).adjustable(adjustments);
                self.ask_own(lane, select, Open::Adjustable(reply), now);
            }
            Event::Line {
                prompt,
                hidden,
                reply,
            } => {
                let select = questions::line(&prompt, hidden);
                self.ask_own(lane, select, Open::Line(reply), now);
            }
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
            Event::Dismiss => self.dismiss(lane, now),
            Event::Key(key) => return self.key(key, now),
            Event::Paste(text) => match self.questions.front_mut() {
                Some(asked) => asked.select.paste(&text),
                // A dropped file arrives as a path; other clipboard text is shown as [Paste].
                None => {
                    if Composer::is_path_list(&text, &attach::resolve) {
                        self.composer.paste(&text);
                        self.adopt_paths();
                    } else {
                        self.composer.paste_marked(&text);
                    }
                }
            },
            Event::Resize(width, height) => {
                self.width = width as usize;
                self.height = height as usize;
            }
            Event::Quit => return Some(Action::Quit),
            // A session's own events reach it through [`App::on`].
            _ => {}
        }
        None
    }

    /// An entry for the session: now if it waits for one, else after what is queued.
    /// While a task works, a plain message goes to it at its next step; a command, a picture or
    /// Tab waits for the task to end.
    pub(super) fn send(&mut self, entry: Typed) {
        // What the person types is for the lane: it is shown again to see it go there.
        let lane = self.lane_id().to_string();
        self.show(&lane);
        let busy = self.busy().is_some();
        let lane = self.lane_mut();
        match lane.waiting.take() {
            Some(reply) => {
                if reply.send(Some(entry.clone())).is_err() {
                    lane.queued.push_back(entry);
                }
            }
            None if busy && steerable(&entry) => lane.steered.push(entry),
            None => lane.queued.push_back(entry),
        }
    }

    /// Each path in the composer that names a file becomes its mark; a picture is kept a copy of.
    pub(crate) fn adopt_paths(&mut self) {
        let dir = self.pasted.clone();
        let keep = |path: &std::path::Path| attach::keep(path, &dir);
        self.composer.adopt(&attach::resolve, &keep);
    }

    /// The person is leaving: the session's wait ends with nothing.
    pub(crate) fn leave(&mut self) {
        self.left = true;
        for lane in self
            .views
            .sessions
            .iter_mut()
            .filter_map(|v| v.lane.as_mut())
        {
            if let Some(reply) = lane.waiting.take() {
                let _ = reply.send(None);
            }
        }
        self.close_questions();
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
