//! The list under the composer as it takes keys: ↓ from an empty prompt gives it the keys, ↑ and
//! ↓ choose a row, Enter or → shows a session, `x` stops a session, an agent or a job, and Esc or ← (or ↑
//! past the top) give the keys back to the composer.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use protocol::Command;

use super::app::{Action, App};
use super::event::Out;
use super::search::stepped;

impl App {
    /// The list's last row drawn: the jobs past the first few are only counted.
    pub(super) fn last_row(&self) -> usize {
        let rows = self.views.order().len() + self.views.jobs.len().min(super::tree::ROWS);
        rows.saturating_sub(1)
    }

    /// Whether ↓ on the composer moves into the list: it is empty, offers nothing, and there
    /// are agents or jobs to choose.
    pub(super) fn enters_list(&self, key: &KeyEvent) -> bool {
        key.code == KeyCode::Down
            && key.modifiers.is_empty()
            && (!self.views.order().is_empty() || !self.views.jobs.is_empty())
            && self.composer.is_empty()
            && self.offers().is_empty()
    }

    /// Give the list the keys, on the session shown.
    pub(super) fn enter_list(&mut self) {
        let shown = self.views.find(&self.views.shown).unwrap_or(0);
        let row = self.views.order().iter().position(|&at| at == shown);
        self.views.focus = Some(row.unwrap_or(0));
    }

    /// Stop the task of the lane `id`, once, and never quit for it: a lane with no task (or
    /// already stopping) is left as it is.
    pub(super) fn halt_lane(&mut self, id: &str) {
        let Some(at) = self.views.find(id) else {
            return;
        };
        let view = &mut self.views.sessions[at];
        let Some(lane) = view.lane.as_deref_mut() else {
            return;
        };
        if view.busy.is_none() || lane.stopping {
            return;
        }
        lane.stopping = true;
        self.outbox.push(Out::Call(Command::Halt { id: id.into() }));
    }

    /// A key while the list has the keys; anything that is not the list's goes to the composer.
    pub(super) fn list_key(&mut self, key: KeyEvent, row: usize, now: Instant) -> Option<Action> {
        let plain = key.modifiers.is_empty();
        let order = self.views.order();
        let session = order.get(row).map(|&at| &self.views.sessions[at]);
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "crossterm's keys, a foreign enum: any other leaves the list"
        )]
        match key.code {
            KeyCode::Up if row == 0 => self.views.focus = None,
            KeyCode::Up => self.views.focus = Some(row - 1),
            KeyCode::Down => self.views.focus = Some(stepped(row, self.last_row(), key.code)),
            KeyCode::Enter | KeyCode::Right => {
                if let Some(id) = session.map(|view| view.id.clone()) {
                    self.views.focus = None;
                    self.show(&id);
                }
            }
            KeyCode::Left => {
                let lane = self.lane_id().to_string();
                self.show(&lane);
                self.views.focus = None;
            }
            // Esc, or Ctrl-C as it closes whatever is open, gives the keys back; nothing stops.
            KeyCode::Esc => self.views.focus = None,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.views.focus = None;
            }
            KeyCode::Char('x') if plain => match session {
                Some(agent) if agent.parent.is_some() && agent.busy.is_some() => {
                    let id = agent.id.clone();
                    self.outbox.push(Out::Call(Command::Halt { id }));
                }
                Some(lane) => {
                    let id = lane.id.clone();
                    self.halt_lane(&id);
                }
                None => {
                    if let Some(job) = self.views.jobs.get(row - order.len()) {
                        let (lane, job) = (job.lane.clone(), job.id.clone());
                        self.outbox.push(Out::Call(Command::StopJob { lane, job }));
                    }
                }
            },
            _ => {
                self.views.focus = None;
                return self.key(key, now);
            }
        }
        None
    }
}
