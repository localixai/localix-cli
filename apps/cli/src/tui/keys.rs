//! The keys the interface takes outside a question: stopping and quitting, sending, the
//! plan-mode switch, and the commands `/` offers.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{Action, App, QUIT_WINDOW_MS, SHIFT_TAB};
use super::composer::Edit;
use super::questions;

impl App {
    pub(super) fn key(&mut self, key: KeyEvent, now: Instant) -> Option<Action> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if let Some((select, _)) = &mut self.select {
            let picked = select.key(key)?;
            let (_, open) = self.select.take()?;
            let stop = ctrl && key.code == KeyCode::Char('c');
            self.declined = questions::answer(open, picked);
            return stop.then(|| self.stop());
        }
        if !(ctrl && key.code == KeyCode::Char('c')) {
            self.quitting = None;
        }
        match key.code {
            KeyCode::Char('c') if ctrl => {
                if self.busy.is_some() {
                    return Some(self.stop());
                }
                if !self.composer.is_empty() {
                    self.composer.clear();
                    return None;
                }
                let again = self
                    .quitting
                    .is_some_and(|at| now.duration_since(at).as_millis() < QUIT_WINDOW_MS);
                if again {
                    return Some(Action::Quit);
                }
                self.quitting = Some(now);
            }
            KeyCode::Char('d') if ctrl && self.composer.is_empty() && self.busy.is_none() => {
                return Some(Action::Quit);
            }
            KeyCode::Esc if self.busy.is_some() && !self.stopping => return Some(self.stop()),
            KeyCode::BackTab => self.send(SHIFT_TAB.into()),
            KeyCode::Tab => self.complete(),
            KeyCode::Up | KeyCode::Down if !self.offers().is_empty() => {
                let last = self.offers().len() - 1;
                self.offered = match key.code {
                    KeyCode::Up => self.offered.saturating_sub(1),
                    _ => (self.offered + 1).min(last),
                };
            }
            KeyCode::Enter if !self.offers().is_empty() && key.modifiers.is_empty() => {
                self.complete();
                let entry = self.composer.text();
                self.composer.clear();
                self.send(entry.trim_end().to_string());
            }
            _ => {
                if let Edit::Sent(entry) = self.composer.key(key)
                    && !entry.trim().is_empty()
                {
                    self.send(entry);
                }
                self.offered = self.offered.min(self.offers().len().saturating_sub(1));
            }
        }
        None
    }

    fn stop(&mut self) -> Action {
        self.stopping = true;
        Action::Stop
    }

    /// The commands the composer's text could be the start of: only while it is `/` and a name.
    pub fn offers(&self) -> Vec<&(String, String)> {
        let text = self.composer.text();
        let Some(typed) = text.strip_prefix('/') else {
            return vec![];
        };
        if typed.contains(char::is_whitespace) {
            return vec![];
        }
        self.commands
            .iter()
            .filter(|(name, _)| name.starts_with(typed))
            .collect()
    }

    fn complete(&mut self) {
        let offers = self.offers();
        if let Some((name, _)) = offers.get(self.offered.min(offers.len().saturating_sub(1))) {
            let name = format!("/{name}");
            self.composer.set(&name);
        }
        self.offered = 0;
    }
}
