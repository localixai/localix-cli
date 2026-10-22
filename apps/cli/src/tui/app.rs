//! The session's interface as state: what has finished (and is yet to be printed), what runs now,
//! what the person is typing, and the question open. It takes events and keys, says what the
//! person wants done (send an entry, stop the task, quit), and is drawn by [`super::live`].
//! Nothing here touches the terminal, so the tests drive it as the person would.

use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::time::Instant;

use contracts::person::Answer;
use ratatui::text::Line;
use serde_json::Value;

use super::cells::{self, End};
use super::composer::Composer;
use super::event::{Approval, Event};
use super::questions;
use super::select::{Picked, Select};
use super::tool_cell::{self, State};

/// What the person asked for that the interface cannot do itself.
#[derive(Debug, PartialEq)]
pub enum Action {
    /// Stop the task running.
    Stop,
    /// Leave the session now.
    Quit,
}

/// What Shift+Tab sends: the session reads it as the plan-mode switch.
pub const SHIFT_TAB: &str = "\u{1b}[Z";

/// A question open, and where its answer goes.
pub enum Open {
    Approval(Sender<Approval>),
    Question(Sender<Option<Answer>>),
    Choice(Sender<Option<usize>>),
    Line(Sender<Option<String>>),
}

pub struct App {
    pub width: usize,
    pub height: usize,
    /// Finished blocks not printed yet.
    pending: Vec<Vec<Line<'static>>>,
    pub composer: Composer,
    /// The commands `/` offers: each name and what it does.
    pub commands: Vec<(String, String)>,
    /// The command marked among those offered.
    pub offered: usize,
    pub mode: String,
    /// When the task running started.
    pub busy: Option<Instant>,
    /// The call running, with when it started.
    pub running: Option<(String, Value, Instant)>,
    /// The latest actions of the browser's hands under the call running.
    pub steps: Vec<String>,
    /// Entries sent while the session was busy, oldest first.
    pub queued: VecDeque<String>,
    /// The session waits for its next entry here.
    pub(super) waiting: Option<Sender<Option<String>>>,
    pub select: Option<(Select, Open)>,
    /// Ctrl-C at an empty prompt: a second one quits, until this moment.
    pub quitting: Option<Instant>,
    /// The person stopped the task running.
    pub stopping: bool,
    /// The person left: the session's next wait ends with nothing.
    left: bool,
    /// The person said no to the call running, and what they said.
    pub(super) declined: Option<String>,
}

/// Rows of the hands' latest actions shown under their call.
const STEPS: usize = 3;
/// How long a first Ctrl-C at the prompt waits for a second.
pub(super) const QUIT_WINDOW_MS: u128 = 2000;

impl App {
    pub fn new(width: usize, height: usize, commands: Vec<(String, String)>) -> Self {
        Self {
            width,
            height,
            pending: vec![],
            composer: Composer::default(),
            commands,
            offered: 0,
            mode: "default".into(),
            busy: None,
            running: None,
            steps: vec![],
            queued: VecDeque::new(),
            waiting: None,
            select: None,
            quitting: None,
            stopping: false,
            left: false,
            declined: None,
        }
    }

    /// The finished blocks to print, with a blank row between them; they are printed once.
    pub fn take_history(&mut self) -> Vec<Line<'static>> {
        let mut out = vec![];
        for block in self.pending.drain(..) {
            out.extend(block);
            out.push(Line::default());
        }
        out
    }

    fn finished(&mut self, block: Vec<Line<'static>>) {
        self.pending.push(block);
    }

    pub fn event(&mut self, event: Event, now: Instant) -> Option<Action> {
        let width = self.width;
        match event {
            Event::Header(facts) => {
                let (version, facts) = facts
                    .split_first()
                    .map_or(("", &[][..]), |(v, f)| (v.as_str(), f));
                let facts: Vec<&str> = facts.iter().map(String::as_str).collect();
                self.finished(cells::header(version, &facts, width));
            }
            Event::Note(text) => self.finished(cells::note(&text, width)),
            Event::Error(text) => self.finished(cells::error(&text, width)),
            Event::Mode(mode) => self.mode = mode,
            Event::Ready(reply) if self.left => {
                let _ = reply.send(None);
            }
            Event::Ready(reply) => match self.queued.pop_front() {
                Some(entry) => {
                    let _ = reply.send(Some(entry));
                }
                None => self.waiting = Some(reply),
            },
            Event::TaskStarted(goal) => {
                if goal != SHIFT_TAB {
                    self.finished(cells::user(&goal, width));
                }
                self.busy = Some(now);
                self.stopping = false;
            }
            Event::Call { name, args } => {
                self.running = Some((name, args, now));
                self.steps.clear();
            }
            Event::Result {
                name,
                args,
                outcome,
            } => {
                self.running = None;
                self.steps.clear();
                let block = match (self.declined.take(), &outcome) {
                    (Some(said), Err(_)) => tool_cell::declined(&name, &args, &said, width),
                    _ => tool_cell::lines(&name, &args, State::Done(&outcome), width),
                };
                self.finished(block);
            }
            Event::Step(step) => {
                self.steps.push(step);
                let over = self.steps.len().saturating_sub(STEPS);
                self.steps.drain(..over);
            }
            Event::Answer(text) if !text.trim().is_empty() => {
                self.finished(cells::agent(&text, width))
            }
            Event::Answer(_) => {}
            Event::TaskEnded {
                status,
                millis,
                turns,
                cost,
            } => {
                self.busy = None;
                self.running = None;
                self.stopping = false;
                let end = End {
                    status: &status,
                    millis,
                    turns,
                    cost,
                };
                self.finished(cells::end(&end, width));
            }
            Event::Approve {
                tool,
                summary,
                reply,
            } => {
                self.select = Some((questions::approval(&tool, &summary), Open::Approval(reply)));
            }
            Event::Ask {
                question,
                at,
                of,
                reply,
            } => {
                self.select = Some((
                    questions::question(&question, at, of),
                    Open::Question(reply),
                ));
            }
            Event::Choose {
                title,
                options,
                marked,
                reply,
            } => {
                let select = questions::choice(&title, &options, marked);
                self.select = Some((select, Open::Choice(reply)));
            }
            Event::Line {
                prompt,
                hidden,
                reply,
            } => {
                self.select = Some((questions::line(&prompt, hidden), Open::Line(reply)));
            }
            Event::Dismiss => {
                if let Some((_, open)) = self.select.take() {
                    self.declined = questions::answer(open, Picked::Cancelled);
                }
            }
            Event::Key(key) => return self.key(key, now),
            Event::Paste(text) => match &mut self.select {
                Some((select, _)) => select.paste(&text),
                None => self.composer.paste(&text),
            },
            Event::Resize(width, height) => {
                self.width = width as usize;
                self.height = height as usize;
            }
            Event::Quit => return Some(Action::Quit),
        }
        None
    }

    /// An entry for the session: now if it waits for one, else after what is queued.
    pub(super) fn send(&mut self, entry: String) {
        match self.waiting.take() {
            Some(reply) => {
                if reply.send(Some(entry.clone())).is_err() {
                    self.queued.push_back(entry);
                }
            }
            None => self.queued.push_back(entry),
        }
    }

    /// The person is leaving: the session's wait ends with nothing.
    pub fn leave(&mut self) {
        self.left = true;
        if let Some(reply) = self.waiting.take() {
            let _ = reply.send(None);
        }
        if let Some((_, open)) = self.select.take() {
            self.declined = questions::answer(open, Picked::Cancelled);
        }
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
