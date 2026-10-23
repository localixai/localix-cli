//! How the sessions' events reach their views (`views.rs`): each session of the interface (a
//! lane) and the agents they started. Only the one shown goes to the terminal; showing
//! another clears the screen and its scrollback and draws that one's transcript from the start,
//! so what scrolls up is always one session's. The list's keys are in `list.rs`.

use std::time::Instant;

use contracts::tool::{ToolError, ToolOutput};
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::app::{Action, App, SHIFT_TAB};
use super::cells::{self, End};
use super::event::{Event, Typed};
use super::style;
use super::tool_cell::{self, State};
pub use super::views::{AgentState, LaneState, SessionView, Views};
use crate::printable::printable;

/// Rows of the hands' latest actions shown under their call.
const STEPS: usize = 3;

/// A finished call as a block: the person's refusal when the call failed on it (the gate's
/// words), which uses up what they said; otherwise its outcome, and what they said waits for the
/// call they refused (calls of several sessions finish in any order).
pub fn result_block(
    declined: &mut Option<String>,
    name: &str,
    args: &Value,
    outcome: &Result<ToolOutput, ToolError>,
    width: usize,
) -> Vec<Line<'static>> {
    match (declined.take(), outcome) {
        (Some(said), Err(ToolError::Failed(why))) if why.contains(crate::gate::DECLINED) => {
            tool_cell::declined(name, args, &said, width)
        }
        (said, _) => {
            *declined = said;
            tool_cell::lines(name, args, State::Done(outcome), width)
        }
    }
}

/// The call `name` with `args` is no longer running.
fn finish_call(running: &mut Vec<(String, Value, Instant)>, name: &str, args: &Value) {
    if let Some(at) = running.iter().position(|(n, a, _)| n == name && a == args) {
        running.remove(at);
    }
}

impl App {
    /// What happened in the session at `at`: its transcript grows, and so does the screen when it
    /// is the one shown. What is not a session's own goes to the interface as a whole.
    pub(super) fn on(&mut self, at: usize, event: Event, now: Instant) -> Option<Action> {
        let width = self.width;
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
            Event::TaskStarted(goal) => {
                view.busy = Some(now);
                view.state = AgentState::Working;
                if let Some(lane) = &mut view.lane {
                    lane.stopping = false;
                }
                if goal == SHIFT_TAB {
                    return None;
                }
                view.named_by(&goal);
                cells::user(&super::steer::shown(&goal), width)
            }
            Event::Call { name, args } => {
                view.running.push((name, args, now));
                if let Some(lane) = &mut view.lane {
                    lane.steps.clear();
                }
                return None;
            }
            Event::Result {
                name,
                args,
                outcome,
            } => {
                finish_call(&mut view.running, &name, &args);
                if let Some(lane) = &mut view.lane {
                    lane.steps.clear();
                }
                let lane = self.lane_at(at);
                let lane = self.views.sessions[lane].lane.as_deref_mut();
                let mut none = None;
                let declined = lane.map_or(&mut none, |lane| &mut lane.declined);
                result_block(declined, &name, &args, &outcome, width)
            }
            Event::Answer(text) if !text.trim().is_empty() => cells::agent(&text, width),
            Event::Answer(_) => return None,
            Event::TaskEnded {
                status,
                millis,
                turns,
                cost,
            } => {
                view.busy = None;
                view.running.clear();
                view.worked += millis;
                if let Some(lane) = &mut view.lane {
                    // What the task did not get to read is the next thing it is given.
                    for entry in lane.steered.drain(..).rev() {
                        lane.queued.push_front(entry);
                    }
                    lane.stopping = false;
                }
                let end = End {
                    status: &status,
                    millis,
                    turns,
                    cost,
                };
                view.tasks += 1;
                let block = cells::end(&end, width);
                // Its lane hears of an agent's end too, and the session shown of a lane's that
                // is not.
                let parent = view.parent.clone().and_then(|id| self.views.find(&id));
                let name = self.views.sessions[at].title().to_string();
                if let Some(parent) = parent {
                    let line = finished_line("Agent", &name, &status, millis, "");
                    self.push(parent, vec![line]);
                } else if at != self.views.home() {
                    self.views.sessions[at].unseen = true;
                    let line = finished_line("Session", &name, &status, millis, " · ← to open");
                    let shown = self.views.find(&self.views.shown).unwrap_or(0);
                    self.push(shown, vec![line]);
                }
                let view = &mut self.views.sessions[at];
                if view.state != AgentState::Closed {
                    view.state = AgentState::Ended(status);
                }
                block
            }
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
                for block in super::cells::earlier(&turns, width) {
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
                let lane = view.id.clone();
                // Out of one of its agents' views; another lane shown stays shown.
                if self.views.home() == at {
                    self.show(&lane);
                }
                self.views.focus = None;
                let theirs = |session: &SessionView| session.parent.as_ref() == Some(&lane);
                self.views.sessions.retain(|session| !theirs(session));
                return None;
            }
            event @ Event::In(..) => return self.event(event, now),
            event => return self.lane_event(at, event, now),
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

    /// What only a lane has; anything else is the interface's.
    fn lane_event(&mut self, at: usize, event: Event, now: Instant) -> Option<Action> {
        let left = self.left;
        let Some(lane) = self.views.sessions[at].lane.as_deref_mut() else {
            return self.global(at, event, now);
        };
        match event {
            Event::Mode(mode) => lane.mode = mode,
            Event::Ready(reply) if left => {
                let _ = reply.send(None);
            }
            // What came back from the background waits behind the person's own.
            Event::Ready(reply) => match lane.queued.pop_front().or(lane.background.pop_front()) {
                Some(entry) => {
                    let _ = reply.send(Some(entry));
                }
                None => lane.waiting = Some(reply),
            },
            Event::Step(step) => {
                lane.steps.push(step);
                let over = lane.steps.len().saturating_sub(STEPS);
                lane.steps.drain(..over);
            }
            Event::Background(text) => {
                let entry = Typed::from(text.as_str());
                match lane.waiting.take() {
                    Some(reply) => {
                        let _ = reply.send(Some(entry));
                    }
                    None => lane.background.push_back(entry),
                }
            }
            Event::Context(left) => lane.context = Some(left),
            Event::TakeSteered { reply, peek } => self.take_steered(at, &reply, peek),
            event => return self.global(at, event, now),
        }
        None
    }

    /// Show the session `id`: its whole transcript, from the top.
    pub fn show(&mut self, id: &str) {
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
    pub fn take_replay(&mut self) -> bool {
        std::mem::take(&mut self.views.replay)
    }

    /// The session shown.
    pub fn shown(&self) -> &SessionView {
        &self.views.sessions[self.views.find(&self.views.shown).unwrap_or(0)]
    }

    /// The calls running and when the task started, in the view shown.
    pub fn shown_work(&self) -> (&[(String, Value, Instant)], Option<Instant>) {
        let view = self.shown();
        (&view.running, view.busy)
    }

    /// Whether a session works: the spinner turns.
    pub fn moving(&self) -> bool {
        let mut sessions = self.views.sessions.iter();
        sessions.any(|view| view.busy.is_some() || !view.running.is_empty())
    }
}

/// `● Agent "scout" finished · 3m 38s` in its lane's transcript, and `● Session "…" finished ·
/// 3m 08s · ← to open` in the one shown; `after` ends it.
fn finished_line(kind: &str, name: &str, status: &str, millis: u64, after: &str) -> Line<'static> {
    let (dot, _, _, verb) = super::tree::ending(status);
    let took = cells::clock(millis / 1000);
    Line::from(vec![
        Span::styled("● ", dot),
        Span::raw(format!("{kind} \"{}\" {verb}", printable(name))),
        Span::styled(format!(" · {took}{after}"), style::dim()),
    ])
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
