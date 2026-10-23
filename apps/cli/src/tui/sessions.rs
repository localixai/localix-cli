//! The sessions in this project, as ← on an empty prompt lists them: the lanes open (working,
//! then waiting for the person, then idle or done), a new session, then this directory's saved
//! sessions, newest first. ↑↓ choose, Enter or → opens (a lane is shown, a saved session opens
//! in a lane of its own), `x` stops a lane's task without switching to it, Esc or ← close it.
//!
//! ```text
//! • Sessions in this project
//!
//! ❯ ⠹ fix the flaky jobs test   running 1m 12s · 54.1k tokens · 2 agents
//!   ? add tabs to the browser   waiting for you
//!   ● refactor the picker       shown · idle · 3 tasks
//!   + New session
//!        2 h ago   4 tasks  make the release binary smaller
//!
//!   ↑↓ choose · enter open · x stop · esc back
//! ```

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::text::{Line, Span};

use super::app::{Action, App};
use super::cells;
use super::style;
use super::views::AgentState;
use super::wrap::cut;
use crate::desk::Target;
use crate::printable::printable;

/// Rows shown at once; a longer list scrolls with the row chosen.
const WINDOW: usize = 14;

#[derive(Debug, Clone, PartialEq)]
enum Row {
    Live(String),
    New,
    /// A saved session's id, and how it is listed.
    Saved(String, String),
}

/// The list open, and the row chosen.
pub struct SessionList {
    rows: Vec<Row>,
    at: usize,
}

impl App {
    /// Whether ← on the composer opens the list: it is empty and offers nothing.
    pub(super) fn opens_sessions(&self, key: &KeyEvent) -> bool {
        key.code == KeyCode::Left
            && key.modifiers.is_empty()
            && self.composer.is_empty()
            && self.offers().is_empty()
    }

    /// Open the list on the newest lane not shown, else on "New session".
    pub(super) fn open_sessions(&mut self) {
        let rows = self.session_rows();
        let home = &self.views.sessions[self.views.home()].id;
        let newest = (self.views.sessions.iter().rev())
            .find(|view| view.lane.is_some() && view.id != *home)
            .map(|view| Row::Live(view.id.clone()));
        let at = (rows.iter().position(|row| Some(row) == newest.as_ref()))
            .or_else(|| rows.iter().position(|row| *row == Row::New));
        self.listing = Some(SessionList {
            rows,
            at: at.unwrap_or(0),
        });
    }

    /// The saved sessions came: the list open lists them, on the row it was on.
    pub(super) fn saved(&mut self, saved: Vec<(String, String)>) {
        self.saved = saved;
        let Some(list) = &self.listing else { return };
        let on = list.rows.get(list.at).cloned();
        let rows = self.session_rows();
        let at = rows.iter().position(|row| Some(row) == on.as_ref());
        self.listing = Some(SessionList {
            at: at.unwrap_or(0),
            rows,
        });
    }

    fn session_rows(&self) -> Vec<Row> {
        let views = &self.views.sessions;
        let lanes: Vec<usize> = (0..views.len())
            .filter(|&at| views[at].lane.is_some())
            .collect();
        let rank = |at: usize| match () {
            _ if views[at].busy.is_some() => 0,
            _ if self.waits_for_person(at) => 1,
            _ => 2,
        };
        let mut live = lanes.clone();
        live.sort_by_key(|&at| rank(at));
        let mut rows: Vec<Row> = live
            .iter()
            .map(|&at| Row::Live(views[at].id.clone()))
            .collect();
        rows.push(Row::New);
        let saved = self
            .saved
            .iter()
            .filter(|(id, _)| self.views.find(id).is_none());
        rows.extend(saved.map(|(id, label)| Row::Saved(id.clone(), label.clone())));
        rows
    }

    /// A key while the list is open.
    pub(super) fn sessions_key(&mut self, key: KeyEvent, now: Instant) -> Option<Action> {
        let list = self.listing.as_mut()?;
        let last = list.rows.len().saturating_sub(1);
        match key.code {
            KeyCode::Up => list.at = list.at.saturating_sub(1),
            KeyCode::Down => list.at = (list.at + 1).min(last),
            KeyCode::Esc | KeyCode::Left => self.listing = None,
            KeyCode::Enter | KeyCode::Right => {
                let row = list.rows.get(list.at).cloned();
                self.listing = None;
                return match row? {
                    Row::Live(id) => {
                        self.show(&id);
                        None
                    }
                    Row::New => self.opening(Target::New),
                    Row::Saved(id, _) => self.opening(Target::Saved(id)),
                };
            }
            KeyCode::Char('x') if key.modifiers.is_empty() => {
                let Some(Row::Live(id)) = list.rows.get(list.at).cloned() else {
                    return None;
                };
                let at = self.views.find(&id)?;
                let view = &mut self.views.sessions[at];
                let lane = view.lane.as_deref_mut()?;
                if view.busy.is_none() || lane.stopping {
                    return None;
                }
                lane.stopping = true;
                return Some(Action::Halt(id));
            }
            // Ctrl-C and the like are the interface's: the list closes first.
            _ if !key.modifiers.is_empty() => {
                self.listing = None;
                return self.key(key, now);
            }
            _ => {}
        }
        None
    }

    /// The list's rows at `now`.
    pub fn sessions_render(
        &self,
        now: Instant,
        spinner: &dyn Fn(Instant) -> Span<'static>,
    ) -> Vec<Line<'static>> {
        let Some(list) = &self.listing else {
            return vec![];
        };
        let width = self.width.max(20);
        let mut out = vec![
            Line::from(vec![
                Span::styled("• ", style::waiting()),
                Span::styled("Sessions in this project", style::bold()),
            ]),
            Line::default(),
        ];
        let names = (list.rows.iter())
            .filter_map(|row| match row {
                Row::Live(id) => self.views.find(id),
                _ => None,
            })
            .map(|at| super::wrap::width(self.views.sessions[at].title()))
            .max()
            .unwrap_or(0)
            .clamp(12, 28);
        let start =
            (list.at.saturating_sub(WINDOW / 2)).min(list.rows.len().saturating_sub(WINDOW));
        for (at, row) in list.rows.iter().enumerate().skip(start).take(WINDOW) {
            let pointer = match at == list.at {
                true => Span::styled("❯ ", style::bold()),
                false => Span::raw("  "),
            };
            let mut spans = vec![pointer];
            match row {
                Row::Live(id) => spans.extend(self.live_row(id, names, now, spinner)),
                Row::New => spans.push(Span::styled("+ New session", style::bold())),
                Row::Saved(_, label) => spans.push(Span::styled(
                    cut(&printable(label), width.saturating_sub(4)),
                    style::dim(),
                )),
            }
            let line = Line::from(spans);
            out.push(line);
        }
        out.push(Line::default());
        let hint = "  ↑↓ choose · enter open · x stop · esc back";
        out.push(Line::from(Span::styled(hint, style::dim())));
        out
    }

    /// A lane's row: its mark, its name, and where it stands.
    fn live_row(
        &self,
        id: &str,
        names: usize,
        now: Instant,
        spinner: &dyn Fn(Instant) -> Span<'static>,
    ) -> Vec<Span<'static>> {
        let Some(at) = self.views.find(id) else {
            return vec![];
        };
        let view = &self.views.sessions[at];
        let agents = (self.views.sessions.iter())
            .filter(|agent| agent.parent.as_deref() == Some(id))
            .count();
        let worked = view.worked
            + view
                .busy
                .map_or(0, |b| now.duration_since(b).as_millis() as u64);
        let spent = format!(
            "{} · {} tokens",
            cells::clock(worked / 1000),
            super::tree::tokens(view.tokens)
        );
        let mut said = vec![];
        let glyph = match (&view.state, view.busy) {
            (_, Some(since)) => {
                said.push(format!("running {spent}"));
                spinner(since)
            }
            _ if self.waits_for_person(at) => {
                said.push("waiting for you".into());
                Span::styled("?", style::waiting())
            }
            (AgentState::Ended(status), None) => {
                let (style, mark, word, _) = super::tree::ending(status);
                said.extend([word.to_string()]);
                if view.unseen {
                    said.push("unseen".into());
                }
                said.push(spent);
                Span::styled(mark, style)
            }
            _ => {
                said.push("idle".into());
                Span::styled("●", style::code())
            }
        };
        if at == self.views.home() {
            said.insert(0, "shown".into());
        }
        if view.tasks > 0 && view.busy.is_none() {
            said.push(cells::count(view.tasks as u64, "task"));
        }
        if agents > 0 {
            said.push(cells::count(agents as u64, "agent"));
        }
        let name = cut(&printable(view.title()), names);
        let pad = names.saturating_sub(super::wrap::width(&name));
        let room = self.width.saturating_sub(names + 8);
        vec![
            glyph,
            Span::raw(" "),
            Span::styled(name, style::bold()),
            Span::raw(" ".repeat(pad + 2)),
            Span::styled(cut(&said.join(" · "), room), style::dim()),
        ]
    }
}

#[cfg(test)]
#[path = "sessions_tests.rs"]
mod tests;
