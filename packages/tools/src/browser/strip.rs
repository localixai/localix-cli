//! The strip of tabs along the top of the one window: the browser's tabs and the terminal
//! sessions' screens side by side, the one shown marked. The person switches with a click and
//! closes one with its ×; the page says which as one short message, read as data. Titles are
//! what pages and commands say, so they reach the page as JSON and are set as text.

use serde_json::json;

/// The strip's height, in points.
pub(super) const HEIGHT: f64 = 34.0;

/// A tab: one of the browser's (its id) or a terminal session's screen (its panel's id).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Key {
    Web(u32),
    Term(u32),
}

impl Key {
    pub(super) fn name(self) -> String {
        match self {
            Key::Web(id) => format!("w{id}"),
            Key::Term(id) => format!("t{id}"),
        }
    }

    pub(super) fn parse(text: &str) -> Option<Self> {
        let (kind, id) = text.split_at_checked(1)?;
        let id = id
            .parse()
            .ok()
            .filter(|_| id.bytes().all(|b| b.is_ascii_digit()))?;
        match kind {
            "w" => Some(Key::Web(id)),
            "t" => Some(Key::Term(id)),
            _ => None,
        }
    }
}

/// What the person did on the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Click {
    Show(Key),
    Close(Key),
}

/// A message from the strip's page: `show w3`, `close t1`. Anything else is nothing.
pub(super) fn click(body: &str) -> Option<Click> {
    let (verb, key) = body.split_once(' ')?;
    let key = Key::parse(key)?;
    match verb {
        "show" => Some(Click::Show(key)),
        "close" => Some(Click::Close(key)),
        _ => None,
    }
}

/// A tab as the strip lists it.
pub(super) struct Item {
    pub key: Key,
    pub title: String,
}

/// The call that draws the strip: the tabs in order, and the one shown.
pub(super) fn script(items: &[Item], shown: Key) -> String {
    let tabs: Vec<_> = items
        .iter()
        .map(|item| {
            let (kind, empty) = match item.key {
                Key::Web(_) => ("web", "New tab"),
                Key::Term(_) => ("term", "Terminal"),
            };
            let title = item.title.trim();
            let title = if title.is_empty() { empty } else { title };
            json!({"key": item.key.name(), "kind": kind, "title": title})
        })
        .collect();
    format!("S({})", json!({"tabs": tabs, "shown": shown.name()}))
}

/// The strip's page: dressed in the frame's stylesheet, clear over the window's glass.
pub(super) const PAGE: &str = include_str!("strip.html");

#[cfg(test)]
#[path = "strip_tests.rs"]
mod tests;
