//! What the end of a call and of a task adds to a session's transcript, and tells the others.

use std::time::Instant;

use protocol::{Label, Outcome};
use ratatui::text::{Line, Span};

use super::{AgentState, TOOK, finish_call, result_block};
use crate::printable::printable;
use crate::tui::app::App;
use crate::tui::cells::{self, End};
use crate::tui::style;

impl App {
    /// A call of the session at `at` ended: its row, with how long it took.
    pub(super) fn call_ended(
        &mut self,
        at: usize,
        call: &str,
        name: &str,
        label: &Label,
        outcome: &Outcome,
        now: Instant,
    ) -> Vec<Line<'static>> {
        let width = self.width;
        let view = &mut self.views.sessions[at];
        let took = finish_call(&mut view.running, call)
            .map(|started| now.saturating_duration_since(started));
        if let Some(lane) = &mut view.lane {
            lane.steps.clear();
        }
        let lane = self.lane_at(at);
        let lane = self.views.sessions[lane].lane.as_deref_mut();
        let mut none = None;
        let declined = lane.map_or(&mut none, |lane| &mut lane.declined);
        let mut block = result_block(declined, name, label, outcome, width);
        // How long it took, where that is worth a glance.
        if let (Some(took), Some(head)) = (took, block.first_mut())
            && took >= TOOK
        {
            let took = cells::duration(u64::try_from(took.as_millis()).unwrap_or(u64::MAX));
            head.spans
                .push(Span::styled(format!(" · {took}"), style::dim()));
        }
        block
    }

    /// The task of the session at `at` ended: its last row, and a line where it is not seen.
    pub(super) fn task_ended(
        &mut self,
        at: usize,
        status: String,
        millis: u64,
        turns: u64,
        cost: f64,
    ) -> Vec<Line<'static>> {
        let width = self.width;
        let view = &mut self.views.sessions[at];
        view.busy = None;
        view.running.clear();
        view.worked += millis;
        if let Some(lane) = &mut view.lane {
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
}

/// `● Agent "scout" finished · 3m 38s` in its lane's transcript, and `● Session "…" finished ·
/// 3m 08s · ← to open` in the one shown; `after` ends it.
fn finished_line(kind: &str, name: &str, status: &str, millis: u64, after: &str) -> Line<'static> {
    let (dot, _, _, verb) = crate::tui::tree::ending(status);
    let took = cells::clock(millis / 1000);
    Line::from(vec![
        Span::styled("● ", dot),
        Span::raw(format!("{kind} \"{}\" {verb}", printable(name))),
        Span::styled(format!(" · {took}{after}"), style::dim()),
    ])
}
