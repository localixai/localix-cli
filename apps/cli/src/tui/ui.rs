//! The session's side of the interface: what it reports, and how it asks and waits.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use contracts::person::{Answer, Question};

use super::event::{Approval, Event, Typed};

/// How often a waiting question checks whether the task was stopped.
const POLL: Duration = Duration::from_millis(100);

/// The session's side of the interface: cheap to clone, one per thing that reports or asks.
#[derive(Clone)]
pub struct Ui {
    events: Sender<Event>,
    /// The session whose events these are: each is sent inside its [`Event::In`].
    lane: Option<Arc<str>>,
}

impl Ui {
    pub fn new(events: Sender<Event>) -> Self {
        Self { events, lane: None }
    }

    /// The same interface, for what session `id` sends.
    pub fn for_lane(&self, id: &str) -> Self {
        Self {
            events: self.events.clone(),
            lane: Some(id.into()),
        }
    }

    /// Send `event`, as the session's unless it names its own (an agent's).
    pub fn send(&self, event: Event) {
        let event = match (&self.lane, event) {
            (Some(lane), event) if !matches!(event, Event::In(..)) => {
                Event::In(lane.to_string(), Box::new(event))
            }
            (_, event) => event,
        };
        let _ = self.events.send(event);
    }

    /// The person's next entry; `None` when they leave.
    pub fn next_entry(&self) -> Option<Typed> {
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

    /// One of `options` (a label and a note each), starting on `marked`; `None` when cancelled.
    pub fn pick(
        &self,
        title: &str,
        options: Vec<(String, String)>,
        marked: Option<usize>,
    ) -> Option<usize> {
        let (reply, answer) = channel();
        self.send(Event::Choose {
            title: title.into(),
            options,
            marked,
            reply,
        });
        answer.recv().ok().flatten()
    }

    /// A row and its inline value, starting on `marked`; `None` when cancelled.
    pub fn pick_adjustable(
        &self,
        title: &str,
        options: Vec<(String, String)>,
        marked: Option<usize>,
        adjustments: Vec<crate::picker::Adjustment>,
    ) -> Option<(usize, usize)> {
        let (reply, answer) = channel();
        self.send(Event::ChooseAdjustable {
            title: title.into(),
            options,
            marked,
            adjustments,
            reply,
        });
        answer.recv().ok().flatten()
    }

    /// A line the person types after `prompt`, shown as dots when `hidden`; `None` when cancelled,
    /// or closed unanswered once `done` says so.
    pub fn line(
        &self,
        prompt: &str,
        hidden: bool,
        done: Option<&dyn Fn() -> bool>,
    ) -> Option<String> {
        let (reply, answer) = channel();
        self.send(Event::Line {
            prompt: prompt.into(),
            hidden,
            reply,
        });
        self.wait(&answer, done.unwrap_or(&|| false)).flatten()
    }

    /// What the person sent to the task running, taken (and shown as theirs) or, with `peek`,
    /// only looked at.
    pub fn steered(&self, peek: bool) -> Vec<String> {
        let (reply, answer) = channel();
        self.send(Event::TakeSteered { reply, peek });
        answer.recv().unwrap_or_default()
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

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
