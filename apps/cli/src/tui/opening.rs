//! Opening a lane (`/new`, or a row of the list of sessions): at the most lanes, the oldest idle
//! one not shown closes first, and is said in the lane that opens; it can be opened again from
//! the list, as a saved session.

use super::app::{Action, App};
use protocol::{Command, Target};

use super::event::Out;

impl App {
    /// Open `target` in a lane of its own: at the most lanes, the oldest idle one not shown
    /// closes first (it can be opened again from the list); when none is idle, nothing opens.
    pub(super) fn opening(&mut self, target: Target) -> Option<Action> {
        let lanes = self
            .views
            .sessions
            .iter()
            .filter(|view| view.lane.is_some());
        if lanes.count() < self.max_lanes {
            return Some(Action::Open(target));
        }
        let home = self.views.home();
        let views = &self.views.sessions;
        let agents_work = |id: &str| {
            let mut agents = views.iter().filter(|v| v.parent.as_deref() == Some(id));
            agents.any(|agent| agent.busy.is_some())
        };
        let idle = (0..views.len()).find(|&at| {
            let view = &views[at];
            let waits = view.lane.as_ref().is_some_and(|lane| lane.ready);
            at != home && waits && view.busy.is_none() && !agents_work(&view.id)
        });
        let shown = self.views.find(&self.views.shown).unwrap_or(home);
        let Some(at) = idle.filter(|&at| !self.waits_for_person(at)) else {
            let full = format!("{} sessions are working: stop one first", self.max_lanes);
            self.push(shown, super::cells::note(&full, self.width));
            return None;
        };
        let view = self.views.sessions.remove(at);
        let said = format!("closed the oldest idle session \"{}\"", view.title());
        let session = view.id.clone();
        self.outbox.push(Out::Call(Command::Close { session }));
        self.views
            .sessions
            .retain(|agent| agent.parent.as_ref() != Some(&view.id));
        // Said in the lane that opens, which is shown next.
        self.closed = Some(said);
        Some(Action::Open(target))
    }
}
