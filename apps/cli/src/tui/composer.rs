//! Where the person types: a few lines of text with a cursor, the earlier entries a key away, and
//! the slash commands offered as they are typed.
//!
//! Enter sends; Shift+Enter, Alt+Enter or Ctrl+J start a new line, as does a line ending in `\`;
//! a paste keeps its lines. Left and right move by character, with Alt (or Ctrl) by word; Home,
//! End, Ctrl+A and Ctrl+E go to the line's ends; Up and Down move between lines, and past the
//! first or the last one walk the earlier entries. Ctrl+U, Ctrl+K and Ctrl+W delete to the
//! start, to the end and the word before.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthChar;

use super::style;
use super::wrap::width as columns;
use crate::attach::Dropped;

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
    /// A secret: each character is drawn as a dot.
    masked: bool,
    /// The pictures attached, each with the mark that stands for it in the text.
    attached: Vec<(String, Dropped)>,
}

impl Composer {
    pub fn masked(masked: bool) -> Self {
        Self {
            masked,
            ..Self::default()
        }
    }

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

    /// Attach the picture at `path`: a mark for it goes in the text where the cursor is.
    pub fn attach(&mut self, path: PathBuf) {
        let mark = self.mark(&Dropped::Image(path.clone()));
        if self.at > 0 && !self.text[self.at - 1].is_whitespace() {
            self.insert(' ');
        }
        for c in mark.chars() {
            self.insert(c);
        }
        self.insert(' ');
        self.attached.push((mark, Dropped::Image(path)));
    }

    /// Every path in the text that names a file (dropped in, pasted, typed whole) turned into a
    /// mark: `[image: shot.png]` for a picture, `[file: notes.md]` for anything else. `resolve`
    /// says what a word names; `keep` keeps a picture somewhere lasting.
    pub fn adopt(
        &mut self,
        resolve: &dyn Fn(&str) -> Option<Dropped>,
        keep: &dyn Fn(&Path) -> PathBuf,
    ) {
        for (start, end) in words(&self.text).into_iter().rev() {
            let word: String = self.text[start..end].iter().collect();
            let looks = word.contains('/') || word.starts_with('~');
            let Some(dropped) = looks.then(|| resolve(&word)).flatten() else {
                continue;
            };
            // The mark has the name as the person knows it; a picture is sent from its copy.
            let mark = self.mark(&dropped);
            let dropped = match dropped {
                Dropped::Image(path) => Dropped::Image(keep(&path)),
                file => file,
            };
            let at_end = self.at >= end;
            let shift = mark.chars().count() as isize - (end - start) as isize;
            self.text.splice(start..end, mark.chars());
            if at_end {
                self.at = (self.at as isize + shift) as usize;
            } else if self.at > start {
                self.at = start + mark.chars().count();
            }
            self.attached.push((mark, dropped));
        }
    }

    /// The mark for `dropped`, numbered when its name is already taken.
    fn mark(&self, dropped: &Dropped) -> String {
        let (kind, path) = match dropped {
            Dropped::Image(path) => ("image", path),
            Dropped::File(path) => ("file", path),
        };
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().to_string(),
        );
        let mut mark = format!("[{kind}: {name}]");
        let mut n = 2;
        while self.attached.iter().any(|(taken, _)| *taken == mark) {
            mark = format!("[{kind}: {name} #{n}]");
            n += 1;
        }
        mark
    }

    /// What is sent for `sent`: its text with each file's mark turned back into the file's path
    /// (the agent reads it), and the pictures whose marks are still there. A mark deleted from
    /// the text takes its attachment with it.
    pub fn take_attached(&mut self, sent: &str) -> (String, Vec<PathBuf>) {
        let mut text = sent.to_string();
        let mut images = vec![];
        for (mark, dropped) in std::mem::take(&mut self.attached) {
            if !text.contains(mark.as_str()) {
                continue;
            }
            match dropped {
                Dropped::Image(path) => images.push(path),
                Dropped::File(path) => text = text.replace(&mark, &path.display().to_string()),
            }
        }
        (text, images)
    }

    /// The `@` word the cursor is at the end of: where it starts, and what follows the `@`.
    pub fn mention(&self) -> Option<(usize, String)> {
        let start = self.text[..self.at]
            .iter()
            .rposition(|c| c.is_whitespace())
            .map_or(0, |at| at + 1);
        let word: String = self.text[start..self.at].iter().collect();
        word.strip_prefix('@').map(|rest| (start, rest.to_string()))
    }

    /// The text from `from` to the cursor replaced with `with`.
    pub fn replace(&mut self, from: usize, with: &str) {
        self.delete(from, self.at);
        for c in with.chars() {
            self.insert(c);
        }
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
            rows.last_mut()
                .unwrap()
                .push(if self.masked { '•' } else { *c });
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

/// The words of `text` as ranges: split at whitespace, except inside quotes or after a
/// backslash, as a shell (and a terminal writing a dropped file) splits them.
fn words(text: &[char]) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut start = None;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (at, c) in text.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match (*c, quote) {
            ('\\', _) => escaped = true,
            (q, None) if q == '\'' || q == '"' => quote = Some(q),
            (q, Some(open)) if q == open => quote = None,
            // Only the whitespace a shell splits at: a name may hold others (U+202F in a
            // screenshot's name).
            (c, None) if c.is_ascii_whitespace() => {
                if let Some(from) = start.take() {
                    out.push((from, at));
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(at);
    }
    if let Some(from) = start {
        out.push((from, text.len()));
    }
    out
}

#[cfg(test)]
#[path = "composer_tests.rs"]
mod tests;
