//! How the sessions' events reach their views (`views.rs`): each session of the interface (a
//! lane) and the agents they started. Only the one shown goes to the terminal; showing
//! another clears the screen and its scrollback and draws that one's transcript from the start,
//! so what scrolls up is always one session's. The list's keys are in `list.rs`.

use std::time::Instant;

use protocol::{Event, Label, Outcome};
use ratatui::text::{Line, Span};

use super::app::{Action, App};
use super::cells;
use super::style;
use super::tool_cell::{self, State};
use super::views::{AgentState, Running, SessionView};
use crate::tui::printable::printable;

mod ends;

/// A call that took this long says so in its row.
const TOOK: std::time::Duration = std::time::Duration::from_millis(500);

/// A finished call as a block: the person's refusal when the call failed on it (the gate's
/// words), which uses up what they said; otherwise its outcome, and what they said waits for the
/// call they refused (calls of several sessions finish in any order).
fn result_block(
    declined: &mut Option<String>,
    name: &str,
    label: &Label,
    outcome: &Outcome,
    width: usize,
) -> Vec<Line<'static>> {
    match (declined.take(), outcome) {
        (Some(said), Outcome::Failed { declined: true, .. }) => {
            tool_cell::declined(label, &said, width)
        }
        (said, _) => {
            *declined = said;
            tool_cell::lines(name, label, &State::Done(outcome), width)
        }
    }
}

/// The call `call` is no longer running; when it started, if it was.
fn finish_call(running: &mut Vec<Running>, call: &str) -> Option<Instant> {
    let at = running.iter().position(|running| running.call == call)?;
    Some(running.remove(at).since)
}

impl App {
    /// What happened in the session at `at`: its transcript grows, and so does the screen when it
    /// is the one shown. What is not a session's own goes to the interface as a whole.
    pub(super) fn on(&mut self, at: usize, event: Event, now: Instant) -> Option<Action> {
        let width = self.width;
        let event = super::here::event(event, self.files.root());
        let agent = self.views.sessions[at].parent.is_some();
        let view = &mut self.views.sessions[at];
        let block = match event {
            Event::Header(facts) => {
                let (version, facts) = facts
                    .split_first()
                    .map_or(("", &[][..]), |(v, f)| (v.as_str(), f));
                let facts: Vec<&str> = facts.iter().map(String::as_str).collect();
                cells::header(version, &facts, width)
            }
            // In an agent, a message its parent sent it while it worked, read at its next step.
            Event::Note(text) if agent => cells::user(&text, width),
            Event::Note(text) => cells::note(&text, width),
            Event::Error(text) => cells::error(&text, width),
            Event::TaskStarted(goal) => view.started(&goal, now, width)?,
            Event::Call { call, name, label } => {
                view.running.push(Running {
                    call,
                    name,
                    label,
                    since: now,
                });
                if let Some(lane) = &mut view.lane {
                    lane.steps.clear();
                }
                return None;
            }
            Event::Result {
                call,
                name,
                label,
                outcome,
            } => self.call_ended(at, &call, &name, &label, &outcome, now),
            Event::Answer(text) if !text.trim().is_empty() => cells::agent(&text, width),
            Event::Answer(_) => return None,
            Event::TaskEnded {
                status,
                millis,
                turns,
                cost,
            } => self.task_ended(at, status, millis, turns, cost),
            Event::Spent(tokens) => {
                view.tokens += tokens;
                return None;
            }
            Event::Started { id, name } => {
                let parent = Some(view.id.clone());
                if self.views.find(&id).is_none() {
                    let agent = SessionView::new(id, parent, name);
                    self.views.sessions.push(agent);
                }
                return None;
            }
            Event::Earlier(turns) => {
                view.tasks += turns.len();
                if let Some(last) = turns.last() {
                    view.state = AgentState::Ended(last.status.clone());
                }
                for block in cells::earlier(&turns, width) {
                    self.push(at, block);
                }
                return None;
            }
            Event::Updated(update) => {
                if let Some(title) = update.title {
                    view.name = title;
                }
                return None;
            }
            Event::Closed => {
                view.state = AgentState::Closed;
                view.running.clear();
                view.busy = None;
                return None;
            }
            Event::AgentsCleared => {
                self.agents_cleared(at);
                return None;
            }
            Event::LaneEnded => return self.lane_ended(at),
            event @ (Event::Mode(_)
            | Event::Step(_)
            | Event::LaneStarted { .. }
            | Event::Saved(_)
            | Event::Show(_)
            | Event::Total(_)
            | Event::Prefill(_)
            | Event::Context(_)
            | Event::Jobs(_)
            | Event::Queue { .. }
            | Event::Steered(_)
            | Event::Copy(_)
            | Event::Quit) => return self.lane_event(at, event),
        };
        self.push(at, block);
        None
    }

    /// `block` ends the transcript of the session at `at`, and is printed now when it is shown.
    pub(super) fn push(&mut self, at: usize, block: Vec<Line<'static>>) {
        let view = &mut self.views.sessions[at];
        if view.id == self.views.shown {
            self.pending.push(block.clone());
        }
        view.blocks.push(block);
    }

    /// The lane at `at` let its agents go: their views go too.
    fn agents_cleared(&mut self, at: usize) {
        let lane = self.views.sessions[at].id.clone();
        // Out of one of its agents' views; another lane shown stays shown.
        if self.views.home() == at {
            self.show(&lane);
        }
        self.views.focus = None;
        let theirs = |session: &SessionView| session.parent.as_ref() == Some(&lane);
        self.views.sessions.retain(|session| !theirs(session));
    }

    /// Show the session `id`: its whole transcript, from the top.
    pub(crate) fn show(&mut self, id: &str) {
        let Some(at) = self.views.find(id) else {
            return;
        };
        if id == self.views.shown {
            return;
        }
        self.views.shown = id.to_string();
        self.views.replay = true;
        self.views.sessions[at].unseen = false;
        self.release_held();
        let view = &self.views.sessions[at];
        self.pending = match &view.parent {
            None => view.blocks.clone(),
            Some(parent) => {
                let parent = self.views.find(parent).map(|at| &self.views.sessions[at]);
                let parent = parent.map_or("", SessionView::title);
                let mut blocks = vec![heading(&view.name, parent)];
                blocks.extend(view.blocks.iter().cloned());
                blocks
            }
        };
    }

    /// Whether the screen must be cleared before the next frame: the view changed.
    pub(crate) fn take_replay(&mut self) -> bool {
        std::mem::take(&mut self.views.replay)
    }

    /// The session shown.
    pub(crate) fn shown(&self) -> &SessionView {
        &self.views.sessions[self.views.find(&self.views.shown).unwrap_or(0)]
    }

    /// The calls running and when the task started, in the view shown.
    pub(crate) fn shown_work(&self) -> (&[Running], Option<Instant>) {
        let view = self.shown();
        (&view.running, view.busy)
    }

    /// Whether a session works: the spinner turns.
    pub(crate) fn moving(&self) -> bool {
        let mut sessions = self.views.sessions.iter();
        sessions.any(|view| view.busy.is_some() || !view.running.is_empty())
    }
}

/// The first block of an agent's view: which agent it is, and the session that started it.
fn heading(name: &str, parent: &str) -> Vec<Line<'static>> {
    let mut line = vec![
        Span::styled("◆ ", style::code()),
        Span::styled(format!("agent {}", printable(name)), style::bold()),
    ];
    if !parent.is_empty() {
        let by = format!("  ·  started by {}", printable(parent));
        line.push(Span::styled(by, style::dim()));
    }
    vec![Line::from(line)]
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
