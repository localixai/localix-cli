//! The tabs the window has held, newest first, for the strip's history menu: one entry per tab,
//! a browser tab with the last web page it showed, a terminal with its session's title. A closed
//! browser tab stays, to be opened again at its page; what became of a terminal the window says
//! (deck_history.rs). In memory, for as long as the window's host lives.

use std::collections::HashMap;
use std::sync::Mutex;

use super::strip::Key;

/// How many entries are kept.
const KEEP: usize = 50;

/// A tab the window held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Entry {
    /// Its own number: a tab's key is another's once it closed, an entry's number never is.
    pub at: u64,
    pub key: Key,
    pub title: String,
    /// A browser tab's web page.
    pub detail: String,
    /// The project it was of when it closed (an open tab's is its key's now).
    pub group: String,
    /// A browser tab no longer open.
    pub closed: bool,
}

#[derive(Default)]
pub(super) struct History(Mutex<(u64, Vec<Entry>)>);

impl History {
    /// The browser's tabs open now: an id new to the list is a new tab, first; an open one no
    /// longer here closed, kept with its project when it showed a web page.
    pub(super) fn web(&self, ids: &[u32], groups: &HashMap<String, String>) {
        let mut held = crate::lock(&self.0);
        let (next, list) = &mut *held;
        for entry in list.iter_mut().filter(|entry| !entry.closed) {
            if let Key::Web(id) = entry.key
                && !ids.contains(&id)
            {
                entry.closed = true;
                if let Some(group) = groups.get(&entry.key.name()) {
                    entry.group.clone_from(group);
                }
            }
        }
        list.retain(|entry| !(entry.closed && entry.detail.is_empty()));
        for &id in ids {
            let open = |entry: &Entry| entry.key == Key::Web(id) && !entry.closed;
            if !list.iter().any(open) {
                add(next, list, Key::Web(id), String::new());
            }
        }
    }

    /// Tab `key` opened, titled `title`: first, and the one of that key before it gone.
    pub(super) fn opened(&self, key: Key, title: &str) {
        let mut held = crate::lock(&self.0);
        let (next, list) = &mut *held;
        list.retain(|entry| entry.key != key || entry.closed);
        add(next, list, key, title.trim().to_string());
    }

    /// Browser tab `id` showed `url` now: first. Only web pages count: a blank page or a data
    /// address is no place to go back to.
    pub(super) fn loaded(&self, id: u32, url: &str) {
        let mut held = crate::lock(&self.0);
        let list = &mut held.1;
        let open = list.iter().position(|e| e.key == Key::Web(id) && !e.closed);
        if let Some(at) = open {
            let mut entry = list.remove(at);
            if url.starts_with("http://") || url.starts_with("https://") {
                entry.detail = url.to_string();
            }
            list.insert(0, entry);
        }
    }

    /// Open tab `key` says its title.
    pub(super) fn titled(&self, key: Key, title: &str) {
        let mut held = crate::lock(&self.0);
        let open = held.1.iter_mut().find(|e| e.key == key && !e.closed);
        if let Some(entry) = open {
            entry.title = title.trim().to_string();
        }
    }

    pub(super) fn list(&self) -> Vec<Entry> {
        crate::lock(&self.0).1.clone()
    }

    /// Entry `at`, while it is kept.
    pub(super) fn get(&self, at: u64) -> Option<Entry> {
        let held = crate::lock(&self.0);
        held.1.iter().find(|entry| entry.at == at).cloned()
    }
}

/// A new entry, first, numbered after the last.
fn add(next: &mut u64, list: &mut Vec<Entry>, key: Key, title: String) {
    *next += 1;
    let entry = Entry {
        at: *next,
        key,
        title,
        detail: String::new(),
        group: String::new(),
        closed: false,
    };
    list.insert(0, entry);
    list.truncate(KEEP);
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
