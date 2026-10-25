//! The web pages the browser's tabs have shown, newest first, for the strip's history menu.
//! In memory, for as long as the window's host lives; one entry per address.

use std::sync::Mutex;

/// How many pages are kept.
const KEEP: usize = 50;

/// A page that was shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Visit {
    pub url: String,
    pub title: String,
}

#[derive(Default)]
pub(super) struct History(Mutex<Vec<Visit>>);

impl History {
    /// `url` was shown now: first in the list, with the title it had before, if any. Only web
    /// pages count: a blank page or a data address is no place to go back to.
    pub(super) fn visited(&self, url: &str) {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return;
        }
        let mut list = crate::lock(&self.0);
        let title = list
            .iter()
            .find(|visit| visit.url == url)
            .map(|visit| visit.title.clone())
            .unwrap_or_default();
        list.retain(|visit| visit.url != url);
        list.insert(
            0,
            Visit {
                url: url.to_string(),
                title,
            },
        );
        list.truncate(KEEP);
    }

    /// The page at `url` says its title.
    pub(super) fn titled(&self, url: &str, title: &str) {
        if let Some(visit) = crate::lock(&self.0)
            .iter_mut()
            .find(|visit| visit.url == url)
        {
            visit.title = title.trim().to_string();
        }
    }

    pub(super) fn list(&self) -> Vec<Visit> {
        crate::lock(&self.0).clone()
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
