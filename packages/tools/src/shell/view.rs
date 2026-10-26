//! A terminal session as a view the helper acts on (`contracts::view`): its screen is what it
//! sees, and its controls are the input line, where a value is typed, and the keys a terminal
//! program reads (Enter, the arrows, Escape, Tab, Backspace, Ctrl-C), each a button by ref. So a
//! goal reaches a REPL, a prompt or a menu as it reaches a page, and the helper knows no terminal.

use std::hash::{Hash, Hasher};

use contracts::view::{Action, Element, Observation, View, ViewError, ViewResult};

use super::session::{Sessions, Wait};

/// The keys offered as buttons, by their names in `keys`.
const KEYS: [&str; 9] = [
    "Enter",
    "Up",
    "Down",
    "Left",
    "Right",
    "Tab",
    "Escape",
    "Backspace",
    "Ctrl-C",
];
/// How long an action waits for the program to answer.
const SETTLE_MS: u64 = 1500;

const CONVENTIONS: &str = "This is a terminal: the text is its screen. Type with `input` (what \
is typed is not sent until Enter), and press a key with its button (`key:Enter`). A menu moves \
with the Up and Down keys and is chosen with Enter; the marked line (❯, >) is the one chosen. A \
prompt waits at the end of the last line. What the screen held before the goal began stays above \
a line that says so: only what is below it came from this goal's steps, and a step done once is \
not done again. Ctrl-C interrupts the program.";

/// The line a goal's view puts between what the screen held when the goal began and what came
/// since.
pub(super) const BEGAN: &str = "──── the goal began here; below is what came since ────";

pub struct TerminalView<'a> {
    sessions: &'a Sessions,
    id: u32,
    stop: &'a (dyn Fn() -> bool + Sync),
    /// The screen's rows above the cursor's when the goal began.
    before: Vec<String>,
}

impl<'a> TerminalView<'a> {
    /// Session `id` as the view of a goal that begins now, or why it has none.
    pub fn new(
        sessions: &'a Sessions,
        id: u32,
        stop: &'a (dyn Fn() -> bool + Sync),
    ) -> Result<Self, String> {
        let screen = sessions.screen(id)?;
        let mut before = lines(&rows(&screen));
        // The cursor's row is typed into next: what is above it stays as it is.
        before.truncate(cursor_row(&screen).saturating_sub(1));
        Ok(Self {
            sessions,
            id,
            stop,
            before,
        })
    }

    /// The rows with the line between before and since, where the rows begun with still show
    /// (scrolled up, or not); a screen drawn over, or with nothing before, reads as it is.
    fn parted(&self, rows: &str) -> String {
        let now = lines(rows);
        let stable = &self.before;
        let scrolled = (0..stable.len()).find(|&s| {
            let kept = &stable[s..];
            now.len() > kept.len() && now[..kept.len()] == *kept
        });
        match scrolled {
            Some(s) => {
                let at = stable.len() - s;
                format!(
                    "{}\n{BEGAN}\n{}",
                    now[..at].join("\n"),
                    now[at..].join("\n")
                )
            }
            None => rows.to_string(),
        }
    }
}

/// A screen's rows, without the line that says its size and cursor.
fn rows(screen: &str) -> String {
    screen
        .split_once('\n')
        .map_or("", |(_, rows)| rows)
        .to_string()
}

/// The cursor's row, from 1, as the screen's first line says it (`cursor at row R col C`).
fn cursor_row(screen: &str) -> usize {
    let head = screen.lines().next().unwrap_or_default();
    let row = head.split_once("cursor at row ").map(|(_, rest)| rest);
    let digits = row.map(|rest| rest.split(' ').next().unwrap_or_default());
    digits.and_then(|digits| digits.parse().ok()).unwrap_or(0)
}

/// The rows as lines, the empty ones at the end left out.
fn lines(rows: &str) -> Vec<String> {
    let mut lines: Vec<String> = rows
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

impl View for TerminalView<'_> {
    fn conventions(&self) -> &str {
        CONVENTIONS
    }

    fn observe(&self) -> ViewResult<Observation> {
        let screen = (self.sessions.screen(self.id)).map_err(ViewError::Unavailable)?;
        let rows = self.parted(&rows(&screen));
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        rows.hash(&mut hash);
        let button = |key: &str| Element {
            reference: format!("key:{key}"),
            role: "button".into(),
            name: key.into(),
            value: None,
            states: vec![],
            editable: false,
            dialog: false,
            options: vec![],
        };
        let input = Element {
            reference: "input".into(),
            role: "textbox".into(),
            name: "terminal input".into(),
            editable: true,
            ..button("")
        };
        let mut elements = vec![input];
        elements.extend(KEYS.map(button));
        Ok(Observation {
            epoch: format!("{:x}", hash.finish()),
            location: format!("session {}", self.id),
            title: self.sessions.command(self.id).unwrap_or_default(),
            text: rows.clone(),
            outline: rows,
            elements,
            ..Observation::default()
        })
    }

    fn act(&self, _: &str, action: &Action) -> ViewResult<()> {
        let (input, keys): (&str, Vec<String>) = match action {
            Action::Type { reference, text } if reference == "input" => (text, vec![]),
            Action::PressEnter { .. } => ("", vec!["Enter".into()]),
            Action::Click { reference } => match reference.strip_prefix("key:") {
                Some(key) if KEYS.contains(&key) => ("", vec![key.into()]),
                _ => return Err(ViewError::Refused(format!("no control {reference} here"))),
            },
            Action::ScrollUp => ("", vec!["PageUp".into()]),
            Action::ScrollDown => ("", vec!["PageDown".into()]),
            other @ (Action::Type { .. } | Action::Select { .. }) => {
                return Err(ViewError::Refused(format!("a terminal cannot {other:?}")));
            }
        };
        let wait = Wait {
            needle: None,
            yield_ms: Some(SETTLE_MS),
        };
        let typed = (input.as_bytes(), keys.as_slice());
        (self.sessions.press(self.id, typed, wait, self.stop)).map_err(ViewError::Unavailable)
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
