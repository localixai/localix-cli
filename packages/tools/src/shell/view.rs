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
prompt waits at the end of the last line. Ctrl-C interrupts the program.";

pub struct TerminalView<'a> {
    pub sessions: &'a Sessions,
    pub id: u32,
    pub stop: &'a (dyn Fn() -> bool + Sync),
}

impl View for TerminalView<'_> {
    fn conventions(&self) -> &str {
        CONVENTIONS
    }

    fn observe(&self) -> ViewResult<Observation> {
        let screen = (self.sessions.screen(self.id)).map_err(ViewError::Unavailable)?;
        let rows = screen
            .split_once('\n')
            .map_or("", |(_, rows)| rows)
            .to_string();
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
        let typed = self
            .sessions
            .send(self.id, input.as_bytes(), &keys, wait, None, self.stop);
        typed.map(|_| ()).map_err(ViewError::Unavailable)
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
