//! The keys the interface takes outside a question: stopping and quitting, sending, the
//! plan-mode switch, and the commands `/` offers.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{Action, App, QUIT_WINDOW_MS, SHIFT_TAB};
use super::composer::Edit;
use super::event::Typed;
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
            KeyCode::Char('v') if ctrl => return Some(Action::Clipboard),
            KeyCode::Tab => {
                self.complete();
            }
            KeyCode::Up | KeyCode::Down if !self.offers().is_empty() => {
                let last = self.offers().len() - 1;
                self.offered = match key.code {
                    KeyCode::Up => self.offered.saturating_sub(1),
                    _ => (self.offered + 1).min(last),
                };
            }
            KeyCode::Enter if !self.offers().is_empty() && key.modifiers.is_empty() => {
                // A file goes into the text; a command is sent as soon as it is chosen.
                if let Some(Offer::File(_)) = self.complete() {
                    return None;
                }
                let entry = self.composer.text();
                self.composer.clear();
                let (text, images) = self.composer.take_attached(&entry);
                self.send(Typed {
                    text: text.trim_end().to_string(),
                    images,
                });
            }
            _ => {
                if key.code == KeyCode::Char('@') {
                    self.files.load();
                }
                // A path typed or dropped in without a paste becomes its mark before it is sent.
                if key.code == KeyCode::Enter && key.modifiers.is_empty() {
                    self.adopt_paths();
                }
                if let Edit::Sent(entry) = self.composer.key(key)
                    && !entry.trim().is_empty()
                {
                    let (text, images) = self.composer.take_attached(&entry);
                    self.send(Typed { text, images });
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
    /// What the composer's text offers: the commands its `/` could start, or the files its
    /// `@` word could name.
    pub fn offers(&self) -> Vec<Offer> {
        if let Some((_, query)) = self.composer.mention() {
            return self
                .files
                .matching(&query)
                .into_iter()
                .map(Offer::File)
                .collect();
        }
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
            .map(|(name, what)| Offer::Command(name.clone(), what.clone()))
            .collect()
    }

    /// The marked offer put in: a command replaces the text, a file its `@` word.
    fn complete(&mut self) -> Option<Offer> {
        let offers = self.offers();
        let offer = offers
            .get(self.offered.min(offers.len().saturating_sub(1)))
            .cloned();
        match &offer {
            Some(Offer::Command(name, _)) => self.composer.set(&format!("/{name}")),
            Some(Offer::File(path)) => {
                if let Some((start, _)) = self.composer.mention() {
                    self.composer.replace(start, &format!("{path} "));
                }
            }
            None => {}
        }
        self.offered = 0;
        offer
    }
}

/// One thing the composer offers to put in.
#[derive(Debug, Clone, PartialEq)]
pub enum Offer {
    /// A command's name and what it does.
    Command(String, String),
    /// A workspace file's path.
    File(String),
}
