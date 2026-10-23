//! The session's side of the interface: what it reports, and how it asks and waits.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, sync_channel};
use std::time::Duration;

use contracts::person::{Answer, Question};

use super::event::{Approval, Event, Typed};

/// How often a waiting question checks whether the task was stopped.
const POLL: Duration = Duration::from_millis(100);

/// The session's side of the interface: cheap to clone, one per thing that reports or asks.
#[derive(Clone)]
pub(crate) struct Ui {
    events: Sender<Event>,
    /// The session whose events these are: each is sent inside its [`Event::In`].
    lane: Option<Arc<str>>,
}

impl Ui {
    pub(crate) fn new(events: Sender<Event>) -> Self {
        Self { events, lane: None }
    }

    /// The same interface, for what session `id` sends.
    pub(crate) fn for_lane(&self, id: &str) -> Self {
        Self {
            events: self.events.clone(),
            lane: Some(id.into()),
        }
    }

    /// Send `event`, as the session's unless it names its own (an agent's).
    pub(crate) fn send(&self, event: Event) {
        let event = match (&self.lane, event) {
            (Some(lane), event) if !matches!(event, Event::In(..)) => {
                Event::In(lane.to_string(), Box::new(event))
            }
            (_, event) => event,
        };
        let _ = self.events.send(event);
    }

    // Each question's reply is one send, taken out of the interface's state as it is made: a
    // channel of one never makes the interface wait.

    /// The person's next entry; `None` when they leave.
    pub(crate) fn next_entry(&self) -> Option<Typed> {
        let (reply, answer) = sync_channel(1);
        self.send(Event::Ready(reply));
        answer.recv().ok().flatten()
    }

    /// Whether an effect may happen; a stopped task closes the question with a no.
    pub(crate) fn approve(
        &self,
        tool: &str,
        summary: &str,
        stopped: &dyn Fn() -> bool,
    ) -> Approval {
        let (reply, answer) = sync_channel(1);
        self.send(Event::Approve {
            tool: tool.into(),
            summary: summary.into(),
            reply,
        });
        self.wait(&answer, stopped).unwrap_or(Approval::No(None))
    }

    /// One of the model's questions; `None` when it was skipped or the task stopped.
    pub(crate) fn ask(
        &self,
        question: &Question,
        at: usize,
        of: usize,
        stopped: &dyn Fn() -> bool,
    ) -> Option<Answer> {
        let (reply, answer) = sync_channel(1);
        self.send(Event::Ask {
            question: crate::server::wire::question(question),
            at,
            of,
            reply,
        });
        self.wait(&answer, stopped)
            .flatten()
            .map(crate::server::wire::answer)
    }

    /// One of `options` (a label and a note each), starting on `marked`; `None` when cancelled.
    pub(crate) fn pick(
        &self,
        title: &str,
        options: Vec<(String, String)>,
        marked: Option<usize>,
    ) -> Option<usize> {
        let (reply, answer) = sync_channel(1);
        self.send(Event::Choose {
            title: title.into(),
            options,
            marked,
            reply,
        });
        answer.recv().ok().flatten()
    }

    /// A row and its inline value, starting on `marked`; `None` when cancelled.
    pub(crate) fn pick_adjustable(
        &self,
        title: &str,
        options: Vec<(String, String)>,
        marked: Option<usize>,
        adjustments: Vec<protocol::Adjustment>,
    ) -> Option<(usize, usize)> {
        let (reply, answer) = sync_channel(1);
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
    pub(crate) fn line(
        &self,
        prompt: &str,
        hidden: bool,
        done: Option<&dyn Fn() -> bool>,
    ) -> Option<String> {
        let (reply, answer) = sync_channel(1);
        self.send(Event::Line {
            prompt: prompt.into(),
            hidden,
            reply,
        });
        self.wait(&answer, done.unwrap_or(&|| false)).flatten()
    }

    /// What the person sent to the task running, taken (and shown as theirs) or, with `peek`,
    /// only looked at.
    pub(crate) fn steered(&self, peek: bool) -> Vec<String> {
        let (reply, answer) = sync_channel(1);
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
