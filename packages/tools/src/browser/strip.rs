//! The strip of tabs along the top of the one window: the browser's tabs and the terminal
//! sessions' screens side by side, the one shown marked. The person switches with a click and
//! closes one with its ×; the page says which as one short message, read as data. Titles are
//! what pages and commands say, so they reach the page as JSON and are set as text. With the tabs
//! of several projects in it (one window, several sessions), each project's are drawn together.

use serde_json::json;

use super::history::Visit;

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
    Back,
    Forward,
    /// The history menu opens or closes.
    History,
    /// The page at this place of the history menu.
    Visit(usize),
}

/// A message from the strip's page: `show w3`, `close t1`, `back`, `forward`, `history`,
/// `visit 2`. Anything else is nothing.
pub(super) fn click(body: &str) -> Option<Click> {
    let (verb, rest) = match body.split_once(' ') {
        Some((verb, rest)) => (verb, Some(rest)),
        None => (body, None),
    };
    match (verb, rest) {
        ("back", None) => Some(Click::Back),
        ("forward", None) => Some(Click::Forward),
        ("history", None) => Some(Click::History),
        ("visit", Some(at)) if at.bytes().all(|b| b.is_ascii_digit()) => {
            at.parse().ok().map(Click::Visit)
        }
        ("show", Some(key)) => Key::parse(key).map(Click::Show),
        ("close", Some(key)) => Key::parse(key).map(Click::Close),
        _ => None,
    }
}

/// A tab as the strip lists it.
pub(super) struct Item {
    pub key: Key,
    pub title: String,
    /// The project it was opened for, as the app names it.
    pub group: String,
}

/// `items` with each group's together, the groups in the order they first appear.
pub(super) fn by_group<T>(mut items: Vec<T>, group: impl Fn(&T) -> &str) -> Vec<T> {
    let mut order: Vec<String> = vec![];
    for item in &items {
        if !order.iter().any(|seen| seen == group(item)) {
            order.push(group(item).to_string());
        }
    }
    items.sort_by_key(|item| order.iter().position(|seen| seen == group(item)));
    items
}

/// The call that draws the strip: the tabs in order, and the one shown. With tabs of more than
/// one project, each project's are drawn together, its name before the first.
pub(super) fn script(items: &[Item], shown: Key) -> String {
    let several = items.iter().any(|item| item.group != items[0].group);
    let mut last = None;
    let tabs: Vec<_> = by_group(items.iter().collect(), |item| item.group.as_str())
        .into_iter()
        .map(|item| {
            let (kind, empty) = match item.key {
                Key::Web(_) => ("web", "New tab"),
                Key::Term(_) => ("term", "Terminal"),
            };
            let title = item.title.trim();
            let title = if title.is_empty() { empty } else { title };
            let mut tab = json!({"key": item.key.name(), "kind": kind, "title": title});
            if several && last.replace(&item.group) != Some(&item.group) {
                tab["group"] = json!(item.group);
            }
            tab
        })
        .collect();
    format!("S({})", json!({"tabs": tabs, "shown": shown.name()}))
}

/// The call that opens the history menu on `visits` (newest first), or closes it.
pub(super) fn menu(visits: Option<&[Visit]>) -> String {
    let shown = visits.map(|visits| {
        let rows: Vec<_> = visits
            .iter()
            .map(|visit| json!({"url": visit.url, "title": visit.title}))
            .collect();
        rows
    });
    format!("M({})", json!(shown))
}

/// The strip's page: dressed in the frame's stylesheet, clear over the window's glass.
pub(super) const PAGE: &str = include_str!("strip.html");

#[cfg(test)]
#[path = "strip_tests.rs"]
mod tests;
