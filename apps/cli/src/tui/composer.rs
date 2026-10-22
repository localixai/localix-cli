//! Where the person types: a few lines of text with a cursor, the earlier entries a key away, and
//! the slash commands offered as they are typed.
//!
//! Enter sends; Shift+Enter, Alt+Enter or Ctrl+J start a new line, as does a line ending in `\`;
//! a paste keeps its lines. Left and right move by character, with Alt (or Ctrl) by word; Home,
//! End, Ctrl+A and Ctrl+E go to the line's ends; Up and Down move between lines, and past the
//! first or the last one walk the earlier entries. Ctrl+U, Ctrl+K and Ctrl+W delete to the
//! start, to the end and the word before.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

use super::style;
use super::wrap::width as columns;

/// What a key did.
#[derive(Debug, PartialEq)]
pub enum Edit {
    /// The text changed, or the cursor moved.
    Changed,
    /// The person sent the text.
    Sent(String),
    /// Not a key the composer takes.
    Ignored,
}

#[derive(Default)]
pub struct Composer {
    text: Vec<char>,
    /// An index into `text`.
    at: usize,
    /// What was sent before, oldest first.
    earlier: Vec<String>,
    /// While walking `earlier`: which one is shown, and what was typed before the walk began.
    walking: Option<(usize, String)>,
}

impl Composer {
    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn clear(&mut self) {
        self.set("");
    }

    pub fn set(&mut self, text: &str) {
        self.text = text.chars().collect();
        self.at = self.text.len();
    }

    /// Text that arrived as one paste: its lines are kept, nothing is sent.
    pub fn paste(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        for c in text
            .chars()
            .filter(|c| *c == '\n' || *c == '\t' || !c.is_control())
        {
            self.insert(if c == '\t' { ' ' } else { c });
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Edit {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Enter if shift || alt => self.insert('\n'),
            KeyCode::Char('j') if ctrl => self.insert('\n'),
            KeyCode::Enter => return self.send(),
            KeyCode::Char('a') if ctrl => self.at = self.line_start(),
            KeyCode::Char('e') if ctrl => self.at = self.line_end(),
            KeyCode::Char('u') if ctrl => self.delete(self.line_start(), self.at),
            KeyCode::Char('k') if ctrl => self.delete(self.at, self.line_end()),
            KeyCode::Char('w') if ctrl => self.delete(self.word_back(), self.at),
            KeyCode::Char('b') if alt => self.at = self.word_back(),
            KeyCode::Char('f') if alt => self.at = self.word_forward(),
            KeyCode::Char(_) if ctrl => return Edit::Ignored,
            KeyCode::Char(c) => self.insert(c),
            KeyCode::Backspace if alt || ctrl => self.delete(self.word_back(), self.at),
            KeyCode::Backspace => self.delete(self.at.saturating_sub(1), self.at),
            KeyCode::Delete => self.delete(self.at, (self.at + 1).min(self.text.len())),
            KeyCode::Left if alt || ctrl => self.at = self.word_back(),
            KeyCode::Right if alt || ctrl => self.at = self.word_forward(),
            KeyCode::Left => self.at = self.at.saturating_sub(1),
            KeyCode::Right => self.at = (self.at + 1).min(self.text.len()),
            KeyCode::Home => self.at = self.line_start(),
            KeyCode::End => self.at = self.line_end(),
            KeyCode::Up => self.up(),
            KeyCode::Down => self.down(),
            _ => return Edit::Ignored,
        }
        Edit::Changed
    }

    /// The rows the composer takes at `width`, after `prompt`, and where its cursor is among them.
    /// The text breaks at the width wherever it reaches it, so every character keeps its column.
    pub fn render(
        &self,
        prompt: Span<'static>,
        placeholder: &str,
        width: usize,
    ) -> (Vec<Line<'static>>, (usize, usize)) {
        let indent = columns(&prompt.content);
        let room = width.saturating_sub(indent).max(1);
        let mut rows: Vec<String> = vec![String::new()];
        let mut used = 0;
        let mut cursor = (0, indent);
        for (index, c) in self.text.iter().enumerate() {
            if *c == '\n' {
                if index == self.at {
                    cursor = (rows.len() - 1, indent + used);
                }
                rows.push(String::new());
                used = 0;
                continue;
            }
            let w = c.width().unwrap_or(0);
            if used + w > room {
                rows.push(String::new());
                used = 0;
            }
            if index == self.at {
                cursor = (rows.len() - 1, indent + used);
            }
            rows.last_mut().unwrap().push(*c);
            used += w;
        }
        if self.at == self.text.len() {
            if used >= room {
                rows.push(String::new());
                used = 0;
            }
            cursor = (rows.len() - 1, indent + used);
        }
        let pad = " ".repeat(indent);
        let lines = rows
            .into_iter()
            .enumerate()
            .map(|(at, row)| {
                let lead = if at == 0 {
                    prompt.clone()
                } else {
                    Span::raw(pad.clone())
                };
                match (self.text.is_empty(), at) {
                    (true, 0) => Line::from(vec![
                        lead,
                        Span::styled(placeholder.to_string(), style::dim()),
                    ]),
                    _ => Line::from(vec![lead, Span::raw(row)]),
                }
            })
            .collect();
        (lines, cursor)
    }

    fn send(&mut self) -> Edit {
        // A line ending in `\` goes on on the next, as in a shell.
        if self.at == self.text.len() && self.text.last() == Some(&'\\') {
            self.text.pop();
            self.at -= 1;
            self.insert('\n');
            return Edit::Changed;
        }
        let text = self.text();
        self.clear();
        self.walking = None;
        if !text.trim().is_empty() && self.earlier.last() != Some(&text) {
            self.earlier.push(text.clone());
        }
        Edit::Sent(text)
    }

    fn insert(&mut self, c: char) {
        self.text.insert(self.at, c);
        self.at += 1;
    }

    fn delete(&mut self, from: usize, to: usize) {
        if from < to {
            self.text.drain(from..to);
            self.at = from;
        }
    }

    fn line_start(&self) -> usize {
        self.text[..self.at]
            .iter()
            .rposition(|c| *c == '\n')
            .map_or(0, |at| at + 1)
    }

    fn line_end(&self) -> usize {
        self.text[self.at..]
            .iter()
            .position(|c| *c == '\n')
            .map_or(self.text.len(), |at| self.at + at)
    }

    fn word_back(&self) -> usize {
        let mut at = self.at;
        while at > 0 && self.text[at - 1].is_whitespace() {
            at -= 1;
        }
        while at > 0 && !self.text[at - 1].is_whitespace() {
            at -= 1;
        }
        at
    }

    fn word_forward(&self) -> usize {
        let mut at = self.at;
        while at < self.text.len() && self.text[at].is_whitespace() {
            at += 1;
        }
        while at < self.text.len() && !self.text[at].is_whitespace() {
            at += 1;
        }
        at
    }

    fn up(&mut self) {
        let start = self.line_start();
        if start > 0 {
            let column = self.at - start;
            let above = self.text[..start - 1]
                .iter()
                .rposition(|c| *c == '\n')
                .map_or(0, |at| at + 1);
            self.at = (above + column).min(start - 1);
            return;
        }
        let next = match &self.walking {
            Some((0, _)) => return,
            Some((at, _)) => at - 1,
            None if self.earlier.is_empty() => return,
            None => self.earlier.len() - 1,
        };
        let typed = self
            .walking
            .take()
            .map_or_else(|| self.text(), |(_, typed)| typed);
        self.set(&self.earlier[next].clone());
        self.walking = Some((next, typed));
    }

    fn down(&mut self) {
        let end = self.line_end();
        if end < self.text.len() {
            let column = self.at - self.line_start();
            let below_end = self.text[end + 1..]
                .iter()
                .position(|c| *c == '\n')
                .map_or(self.text.len(), |at| end + 1 + at);
            self.at = (end + 1 + column).min(below_end);
            return;
        }
        match self.walking.take() {
            Some((at, typed)) if at + 1 < self.earlier.len() => {
                self.set(&self.earlier[at + 1].clone());
                self.walking = Some((at + 1, typed));
            }
            Some((_, typed)) => self.set(&typed),
            None => {}
        }
    }
}

#[cfg(test)]
#[path = "composer_tests.rs"]
mod tests;
