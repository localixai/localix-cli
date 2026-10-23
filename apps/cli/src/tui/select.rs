//! A choice put to the person: a title, numbered options, and an option that opens a line to type
//! on. Up and Down (or k and j) move, a number picks at once, Enter picks the marked one, and Esc
//! cancels (or, while typing, goes back to the options). No letter picks: words typed ahead as a
//! choice opens must not answer it. A list longer than its window is searchable instead: letters
//! go to a query that filters it (`search`), PageUp, PageDown, Home and End move too, and no
//! number picks.
//!
//! ```text
//! Run this command?
//!   cargo test --workspace
//!
//! ❯ 1. Yes
//!   2. Yes, and don't ask again for bash
//!   3. No, and tell llx what to do differently
//!
//!   ↑↓ to choose · enter to confirm · esc to say no
//! ```

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Line;

use super::composer::{Composer, Edit};
use super::search::{self, Search, Searched};

mod render;

/// Options shown at once; a longer list scrolls with the marked one.
const WINDOW: usize = 12;

pub struct Choice {
    pub label: String,
    /// Said after the label, dim.
    pub detail: String,
    /// Picking it opens a line to type on.
    pub types: bool,
}

impl Choice {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.into(),
            detail: String::new(),
            types: false,
        }
    }

    pub fn detail(mut self, detail: &str) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn types(mut self) -> Self {
        self.types = true;
        self
    }
}

#[derive(Debug, PartialEq)]
pub enum Picked {
    Option(usize),
    Adjusted(usize, usize),
    /// The options checked, in order, where several may be.
    Many(Vec<usize>),
    /// An option that types, with what was typed.
    Typed(usize, String),
    Cancelled,
}

pub struct Select {
    title: Vec<Line<'static>>,
    choices: Vec<Choice>,
    at: usize,
    typing: Option<Composer>,
    /// What Esc does, as the hint says it: `esc to cancel`, `esc to say no`.
    escape: String,
    /// What is typed is a secret: shown as dots.
    hidden: bool,
    /// Only a line to type, with no options shown.
    direct: bool,
    /// Several options may be checked (Space), then sent together (Enter).
    multiple: bool,
    checked: Vec<bool>,
    /// Letters filter the list: a long one.
    search: Option<Search>,
    adjustments: Option<Vec<crate::picker::Adjustment>>,
}

impl Select {
    pub fn new(title: Vec<Line<'static>>, choices: Vec<Choice>, escape: &str) -> Self {
        let choices_len = choices.len();
        Self {
            search: None,
            adjustments: None,
            title,
            choices,
            at: 0,
            typing: None,
            escape: escape.into(),
            hidden: false,
            direct: false,
            multiple: false,
            checked: vec![],
        }
        .searchable(choices_len > WINDOW)
    }

    /// Each original row owns its value even when the search changes its position.
    pub fn adjustable(mut self, mut adjustments: Vec<crate::picker::Adjustment>) -> Self {
        adjustments.resize_with(self.choices.len(), Default::default);
        for adjustment in &mut adjustments {
            adjustment.at = adjustment.at.min(adjustment.values.len().saturating_sub(1));
        }
        self.adjustments = Some(adjustments);
        self
    }

    /// Letters filter the list, and no number picks.
    pub fn searchable(mut self, on: bool) -> Self {
        let rows = self
            .choices
            .iter()
            .map(|c| search::keys(&c.label, &c.detail));
        self.search = on.then(|| Search::new(rows.collect(), &self.title));
        self
    }

    /// The options listed, as positions in the whole list: those that match, best first.
    fn rows(&self) -> Vec<usize> {
        match &self.search {
            Some(search) => search.shown.clone(),
            None => (0..self.choices.len()).collect(),
        }
    }

    pub fn multiple(mut self, multiple: bool) -> Self {
        self.multiple = multiple;
        self.checked = vec![false; self.choices.len()];
        self
    }

    /// The marker starts on `at`.
    pub fn at(mut self, at: usize) -> Self {
        self.at = at.min(self.choices.len().saturating_sub(1));
        self
    }

    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// The title without its line `at` (who asked, once that is plain).
    pub fn drop_title_line(&mut self, at: usize) {
        if at < self.title.len() {
            self.title.remove(at);
        }
    }

    /// Opens the line of the option at `at` at once: a question with nothing to pick from.
    pub fn typing(mut self, at: usize) -> Self {
        self.at = at;
        self.typing = Some(Composer::masked(self.hidden));
        // With nothing else to pick, the line is the whole question: no list, and Esc cancels.
        self.direct = self.choices.len() == 1;
        self
    }

    /// What a key did: `None` while the choice is still open.
    pub fn key(&mut self, key: KeyEvent) -> Option<Picked> {
        if let Some(composer) = &mut self.typing {
            return match key.code {
                KeyCode::Esc if self.direct => Some(Picked::Cancelled),
                KeyCode::Esc => {
                    self.typing = None;
                    None
                }
                _ => match composer.key(key) {
                    Edit::Sent(text) => Some(Picked::Typed(self.at, text.trim().to_string())),
                    _ => None,
                },
            };
        }
        let toggles = self.multiple && key.code == KeyCode::Char(' ');
        if let Some(search) = self.search.as_mut().filter(|_| !toggles) {
            match search.key(key) {
                Searched::Changed => {
                    if let Some(first) = search.shown.first().filter(|_| !search.is_empty()) {
                        self.at = *first;
                    }
                    return None;
                }
                Searched::Cancel => return Some(Picked::Cancelled),
                Searched::Ignored => {}
            }
        }
        if matches!(key.code, KeyCode::Left | KeyCode::Right)
            && !self.rows().is_empty()
            && let Some(adjustment) = self
                .adjustments
                .as_mut()
                .and_then(|rows| rows.get_mut(self.at))
        {
            adjustment.at = match key.code {
                KeyCode::Left => adjustment.at.saturating_sub(1),
                _ => (adjustment.at + 1).min(adjustment.values.len().saturating_sub(1)),
            };
            return None;
        }
        let last = self.choices.len().saturating_sub(1);
        let code = match key.code {
            KeyCode::Char('k') => KeyCode::Up,
            KeyCode::Char('j') => KeyCode::Down,
            code => code,
        };
        if let Some(to) = search::moved(&self.rows(), self.at, code, WINDOW) {
            self.at = to;
            return None;
        }
        match key.code {
            KeyCode::Esc => return Some(Picked::Cancelled),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(Picked::Cancelled);
            }
            KeyCode::Char(' ') if self.multiple && !self.choices[self.at].types => {
                self.checked[self.at] = !self.checked[self.at];
            }
            KeyCode::Enter if self.multiple && self.checked.contains(&true) => {
                let picked = (0..self.checked.len())
                    .filter(|at| self.checked[*at])
                    .collect();
                return Some(Picked::Many(picked));
            }
            KeyCode::Enter if self.rows().is_empty() => {}
            KeyCode::Enter => return self.pick(self.at),
            KeyCode::Char(c) => {
                let numbered = c
                    .to_digit(10)
                    .map(|n| n as usize)
                    .filter(|n| (1..=last + 1).contains(n));
                if let Some(n) = numbered {
                    return self.pick(n - 1);
                }
            }
            _ => {}
        }
        None
    }

    /// Text pasted while typing goes on the line, else on the query; elsewhere it is ignored.
    pub fn paste(&mut self, text: &str) {
        if let Some(composer) = &mut self.typing {
            composer.paste(text);
        } else if let Some(search) = &mut self.search {
            search.paste(text);
            self.at = search.shown.first().copied().unwrap_or(self.at);
        }
    }

    fn pick(&mut self, at: usize) -> Option<Picked> {
        self.at = at;
        if self.choices[at].types {
            self.typing = Some(Composer::masked(self.hidden));
            return None;
        }
        match &self.adjustments {
            Some(adjustments) => Some(Picked::Adjusted(at, adjustments[at].at)),
            None => Some(Picked::Option(at)),
        }
    }
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
