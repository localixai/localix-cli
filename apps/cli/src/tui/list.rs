//! The list under the composer as it takes keys: ↓ from an empty prompt gives it the keys, ↑ and
//! ↓ choose a row, Enter or → shows a session, `x` stops a session, an agent or a job, and Esc or ← (or ↑
//! past the top) give the keys back to the composer.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{Action, App};

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

    /// A key while the list has the keys; anything that is not the list's goes to the composer.
    pub(super) fn list_key(&mut self, key: KeyEvent, row: usize, now: Instant) -> Option<Action> {
        let plain = key.modifiers.is_empty();
        let order = self.views.order();
        let session = order.get(row).map(|&at| &self.views.sessions[at]);
        match key.code {
            KeyCode::Up if row == 0 => self.views.focus = None,
            KeyCode::Up => self.views.focus = Some(row - 1),
            KeyCode::Down => self.views.focus = Some((row + 1).min(self.last_row())),
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
                    crate::interrupt::stop(&agent.id);
                }
                Some(lane) if lane.busy.is_some() => {
                    let id = lane.id.clone();
                    let at = self.views.find(&id)?;
                    let state = self.views.sessions[at].lane.as_deref_mut()?;
                    if !state.stopping {
                        state.stopping = true;
                        return Some(Action::Halt(id));
                    }
                }
                Some(_) => {}
                None => {
                    if let Some(job) = self.views.jobs.get(row - order.len()) {
                        crate::interrupt::stop(&crate::interrupt::job_key(&job.lane, &job.id));
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
