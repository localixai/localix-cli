//! A choice put to the person: a title, numbered options, and an option that opens a line to type
//! on. Up and Down (or k and j) move, a number or an option's own key picks at once, Enter picks
//! the marked one, and Esc cancels (or, while typing, goes back to the options).
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
use ratatui::text::{Line, Span};

use super::composer::{Composer, Edit};
use super::style;
use super::wrap::wrap;
use crate::printable::printable;

pub struct Choice {
    pub label: String,
    /// Said after the label, dim.
    pub detail: String,
    /// A key that picks it at once (`y`).
    pub key: Option<char>,
    /// Picking it opens a line to type on.
    pub types: bool,
}

impl Choice {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.into(),
            detail: String::new(),
            key: None,
            types: false,
        }
    }

    pub fn key(mut self, key: char) -> Self {
        self.key = Some(key);
        self
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
}

impl Select {
    pub fn new(title: Vec<Line<'static>>, choices: Vec<Choice>, escape: &str) -> Self {
        Self {
            title,
            choices,
            at: 0,
            typing: None,
            escape: escape.into(),
        }
    }

    /// Opens the line of the option at `at` at once: a question with nothing to pick from.
    pub fn typing(mut self, at: usize) -> Self {
        self.at = at;
        self.typing = Some(Composer::default());
        self
    }

    /// What a key did: `None` while the choice is still open.
    pub fn key(&mut self, key: KeyEvent) -> Option<Picked> {
        if let Some(composer) = &mut self.typing {
            return match key.code {
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
        let last = self.choices.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc => return Some(Picked::Cancelled),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(Picked::Cancelled);
            }
            KeyCode::Up | KeyCode::Char('k') => self.at = self.at.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.at = (self.at + 1).min(last),
            KeyCode::Enter => return self.pick(self.at),
            KeyCode::Char(c) => {
                let numbered = c
                    .to_digit(10)
                    .map(|n| n as usize)
                    .filter(|n| (1..=last + 1).contains(n));
                let keyed = self.choices.iter().position(|choice| choice.key == Some(c));
                if let Some(at) = numbered.map(|n| n - 1).or(keyed) {
                    return self.pick(at);
                }
            }
            _ => {}
        }
        None
    }

    /// Text pasted while typing goes on the line; elsewhere it is ignored.
    pub fn paste(&mut self, text: &str) {
        if let Some(composer) = &mut self.typing {
            composer.paste(text);
        }
    }

    fn pick(&mut self, at: usize) -> Option<Picked> {
        self.at = at;
        if self.choices[at].types {
            self.typing = Some(Composer::default());
            return None;
        }
        Some(Picked::Option(at))
    }

    /// The rows at `width`, and the cursor's place among them while typing.
    pub fn render(&self, width: usize) -> (Vec<Line<'static>>, Option<(usize, usize)>) {
        let mut out = self.title.clone();
        if !out.is_empty() {
            out.push(Line::default());
        }
        for (at, choice) in self.choices.iter().enumerate() {
            let marked = at == self.at;
            let mark = if marked {
                Span::styled("❯ ", style::bold())
            } else {
                Span::raw("  ")
            };
            let label_style = if marked {
                style::bold()
            } else {
                Default::default()
            };
            let mut line = Line::from(vec![
                Span::styled(format!("{}. ", at + 1), label_style),
                Span::styled(printable(&choice.label), label_style),
            ]);
            if !choice.detail.is_empty() {
                line.spans.push(Span::styled(
                    format!(" {}", printable(&choice.detail)),
                    style::dim(),
                ));
            }
            let number = at.to_string().len() + 3;
            out.extend(wrap(
                &line,
                width,
                &Line::from(mark),
                &Line::from(" ".repeat(number + 1)),
            ));
        }
        let mut cursor = None;
        if let Some(composer) = &self.typing {
            out.push(Line::default());
            let (rows, (row, column)) =
                composer.render(Span::styled("  ❯ ", style::bold()), "Type here", width);
            cursor = Some((out.len() + row, column));
            out.extend(rows);
        }
        out.push(Line::default());
        let hint = match self.typing {
            Some(_) => "  enter to send · esc to go back".to_string(),
            None => format!("  ↑↓ to choose · enter to confirm · {}", self.escape),
        };
        out.push(Line::from(Span::styled(hint, style::dim())));
        (out, cursor)
    }
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
