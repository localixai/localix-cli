//! Where each of the server's events goes in the interface: the server's own (a lane started, the
//! sessions listed, a lane shown, the end), a session's (`agents.rs`), what only a lane has, and
//! what is the interface's as a whole. Each stage names what it passes on, so an event the
//! protocol gains is placed in one of them, never dropped unseen (S10).

use std::time::Instant;

use super::app::{Action, App};
use super::cells;
use super::event::{Event, JobRow};
use crate::tui::printable::printable;

/// Rows of the latest steps shown under their call.
const STEPS: usize = 3;

impl App {
    /// What happened in session `id` (a lane or an agent); one no longer shown is let go.
    pub(crate) fn event_in(&mut self, id: &str, event: Event, now: Instant) -> Option<Action> {
        let at = self.views.find(id)?;
        self.on(at, event, now)
    }

    /// What happened in the server as a whole; anything else is the lane shown's.
    pub(crate) fn event(&mut self, event: Event, now: Instant) -> Option<Action> {
        match event {
            Event::LaneStarted { id, name } => {
                self.views.lane_started(id.clone(), &name);
                if let (Some(said), Some(at)) = (self.closed.take(), self.views.find(&id)) {
                    self.push(at, cells::note(&printable(&said), self.width));
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
            event @ (Event::Header(_)
            | Event::Note(_)
            | Event::Error(_)
            | Event::Mode(_)
            | Event::TaskStarted(_)
            | Event::Call { .. }
            | Event::Result { .. }
            | Event::Step(_)
            | Event::Answer(_)
            | Event::TaskEnded { .. }
            | Event::Earlier(_)
            | Event::Updated(_)
            | Event::LaneEnded
            | Event::Started { .. }
            | Event::Closed
            | Event::Spent(_)
            | Event::Total(_)
            | Event::AgentsCleared
            | Event::Prefill(_)
            | Event::Context(_)
            | Event::Jobs(_)
            | Event::Queue { .. }
            | Event::Steered(_)
            | Event::Copy(_)) => self.on(self.views.home(), event, now),
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
            Event::Header(_)
            | Event::Note(_)
            | Event::Error(_)
            | Event::Mode(_)
            | Event::TaskStarted(_)
            | Event::Call { .. }
            | Event::Result { .. }
            | Event::Step(_)
            | Event::Answer(_)
            | Event::TaskEnded { .. }
            | Event::Earlier(_)
            | Event::Updated(_)
            | Event::LaneStarted { .. }
            | Event::LaneEnded
            | Event::Saved(_)
            | Event::Show(_)
            | Event::Started { .. }
            | Event::Closed
            | Event::Spent(_)
            | Event::Total(_)
            | Event::AgentsCleared
            | Event::Context(_)
            | Event::Queue { .. }
            | Event::Steered(_) => {}
        }
        None
    }

    /// What only a lane has; anything else is the interface's.
    pub(super) fn lane_event(&mut self, at: usize, event: Event) -> Option<Action> {
        let Some(lane) = self.views.sessions[at].lane.as_deref_mut() else {
            return self.global(at, event);
        };
        match event {
            Event::Mode(mode) => lane.mode = mode,
            Event::Step(step) => {
                lane.steps.push(step);
                let over = lane.steps.len().saturating_sub(STEPS);
                lane.steps.drain(..over);
            }
            Event::Context(left) => lane.context = Some(left),
            Event::Total(total) => lane.total = Some(total),
            Event::Queue {
                ready,
                steered,
                queued,
                background,
            } => {
                lane.ready = ready;
                lane.steered = steered;
                lane.queued = queued;
                lane.background = background;
            }
            // The task read what the person said to it: theirs now, in the transcript.
            Event::Steered(texts) => {
                for text in texts {
                    let block = cells::user(&text, self.width);
                    self.push(at, block);
                }
            }
            event @ (Event::Header(_)
            | Event::Note(_)
            | Event::Error(_)
            | Event::TaskStarted(_)
            | Event::Call { .. }
            | Event::Result { .. }
            | Event::Answer(_)
            | Event::TaskEnded { .. }
            | Event::Earlier(_)
            | Event::Updated(_)
            | Event::LaneStarted { .. }
            | Event::LaneEnded
            | Event::Saved(_)
            | Event::Show(_)
            | Event::Started { .. }
            | Event::Closed
            | Event::Spent(_)
            | Event::AgentsCleared
            | Event::Prefill(_)
            | Event::Jobs(_)
            | Event::Copy(_)
            | Event::Quit) => return self.global(at, event),
        }
        None
    }
}
