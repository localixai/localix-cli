//! The history menu's side of the window: what became of each tab it lists, and an entry shown
//! again. The window is what says it: a terminal is listed while its screen is kept, a browser
//! tab while it is open or was closed at a web page.

use super::super::history::Entry;
use super::super::host::Shared;
use super::super::panel::Held;
use super::super::strip::{self, Key, Row, State};
use super::{Deck, Shown};

/// What showing an entry of the history again comes to.
#[derive(Debug, PartialEq, Eq)]
pub(in super::super) enum Revisit {
    Shown,
    /// Its page, for the host to open in a new tab of the browser.
    Load(String),
    /// The window no longer has it.
    Gone,
}

impl Deck {
    /// The history's entries the window can show, newest first, a project's together.
    pub(in super::super) fn rows(&self, shared: &Shared) -> Vec<Row> {
        let groups = crate::lock(&shared.groups).clone();
        let list = shared.history.list().into_iter();
        let rows = list.filter_map(|entry| {
            let state = self.state(&entry)?;
            let group = match state {
                State::Closed => entry.group,
                State::Open | State::Hidden | State::Ended => {
                    groups.get(&entry.key.name()).cloned().unwrap_or_default()
                }
            };
            let title = entry.title.strip_prefix("llx · ").unwrap_or(&entry.title);
            Some(Row {
                at: entry.at,
                key: entry.key,
                title: title.to_string(),
                detail: entry.detail,
                state,
                group,
            })
        });
        strip::by_group(rows.collect(), |row| row.group.as_str())
    }

    /// What became of `entry`'s tab, while the window can show it.
    fn state(&self, entry: &Entry) -> Option<State> {
        match entry.key {
            Key::Web(_) if entry.closed => Some(State::Closed),
            Key::Web(id) => {
                let open = self.web.as_ref().filter(|web| web.listed().0.contains(&id));
                open.map(|_| State::Open)
            }
            Key::Term(id) => self.term(id).map(|open| match open.held {
                Held::Live => State::Open,
                Held::Hidden => State::Hidden,
                Held::Ended => State::Ended,
            }),
        }
    }

    /// Show `entry` again, and close the menu: an open tab, a terminal the person closed (a tab
    /// again) or one that ended (its last screen, which is no tab).
    pub(in super::super) fn revisit(&mut self, entry: &Entry, shared: &Shared) -> Revisit {
        self.menu = false;
        match (self.state(entry), entry.key) {
            (None, Key::Web(_) | Key::Term(_)) => Revisit::Gone,
            (Some(State::Closed), Key::Web(_) | Key::Term(_)) => {
                self.show(Shown::Web);
                Revisit::Load(entry.detail.clone())
            }
            (Some(State::Open | State::Hidden | State::Ended), Key::Web(id)) => {
                let switched = self.web.as_mut().map(|web| web.switch(id, shared));
                self.show(Shown::Web);
                match switched {
                    Some(Ok(())) => Revisit::Shown,
                    Some(Err(_)) | None => Revisit::Gone,
                }
            }
            (Some(State::Open | State::Hidden | State::Ended), Key::Term(id)) => {
                self.unhide_term(id);
                self.shown = Shown::Term(id);
                Revisit::Shown
            }
        }
    }
}
