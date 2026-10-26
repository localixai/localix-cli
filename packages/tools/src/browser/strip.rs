//! The strip of tabs along the top of the one window: the browser's tabs and the terminal
//! sessions' screens side by side, the one shown marked. The person switches with a click and
//! closes one with its ×; the page says which as one short message, read as data. Titles are
//! what pages and commands say, so they reach the page as JSON and are set as text. With the tabs
//! of several projects in it (one window, several sessions), a row of the projects goes above
//! them, and the tabs under it are the shown project's.

use serde_json::json;

/// The strip's height, in points.
pub(super) const HEIGHT: f64 = 34.0;
/// The row of projects above it, drawn when the tabs of more than one project are in the window.
pub(super) const ROW: f64 = 28.0;

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
    /// The history menu's entry of this number.
    Visit(u64),
    /// Commenting on the pages turns on or off.
    Comment,
    /// The comments held go to whoever listens, and their marks leave the pages.
    Send,
    /// The strip's empty part is the titlebar: a press moves the window, a double click zooms it.
    Drag,
    Zoom,
}

/// A message from the strip's page: `show w3`, `close t1`, `back`, `forward`, `history`,
/// `visit 2`, `comment`, `send`, `drag`, `zoom`. Anything else is nothing.
pub(super) fn click(body: &str) -> Option<Click> {
    let (verb, rest) = match body.split_once(' ') {
        Some((verb, rest)) => (verb, Some(rest)),
        None => (body, None),
    };
    match (verb, rest) {
        ("back", None) => Some(Click::Back),
        ("forward", None) => Some(Click::Forward),
        ("history", None) => Some(Click::History),
        ("comment", None) => Some(Click::Comment),
        ("send", None) => Some(Click::Send),
        ("drag", None) => Some(Click::Drag),
        ("zoom", None) => Some(Click::Zoom),
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
/// one project, each project's are together and each says its project.
pub(super) fn script(items: &[Item], shown: Key) -> String {
    let several = items.iter().any(|item| item.group != items[0].group);
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
            if several {
                tab["group"] = json!(item.group);
            }
            tab
        })
        .collect();
    format!("S({})", json!({"tabs": tabs, "shown": shown.name()}))
}

/// What became of a tab the history menu lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    Open,
    /// A terminal the person closed: its program runs on, its screen live out of sight.
    Hidden,
    /// A terminal whose session ended: its last screen is kept.
    Ended,
    /// A browser tab closed: its page opens again in a new one.
    Closed,
}

/// An entry of the history menu.
pub(super) struct Row {
    pub at: u64,
    pub key: Key,
    pub title: String,
    /// A browser tab's page.
    pub detail: String,
    pub state: State,
    pub group: String,
}

/// The call that opens the history menu on `rows` (newest first), or closes it.
pub(super) fn menu(rows: Option<&[Row]>) -> String {
    let shown = rows.map(|rows| {
        let rows: Vec<_> = rows
            .iter()
            .map(|row| {
                let kind = match row.key {
                    Key::Web(_) => "web",
                    Key::Term(_) => "term",
                };
                let state = match row.state {
                    State::Open => "open",
                    State::Hidden => "hidden",
                    State::Ended => "ended",
                    State::Closed => "closed",
                };
                json!({"at": row.at, "key": row.key.name(), "kind": kind, "title": row.title,
                    "detail": row.detail, "state": state, "group": row.group})
            })
            .collect();
        rows
    });
    format!("M({})", json!(shown))
}

/// The call that marks the comment button on or off, and offers to send the comments held.
pub(super) fn commenting(on: bool, held: usize) -> String {
    format!("C({on}, {held})")
}

/// The call that turns commenting on or off in a page; a page whose marks are of an earlier
/// batch than `sent` drops them.
pub(super) fn comment(on: bool, sent: u32) -> String {
    format!(
        "window.__dloop_call && window.__dloop_call(0, 'comment', {{ on: {on}, sent: {sent} }})"
    )
}

/// The strip's page: dressed in the frame's stylesheet, clear over the window's glass.
pub(super) const PAGE: &str = include_str!("strip.html");

#[cfg(test)]
#[path = "strip_tests.rs"]
mod tests;
