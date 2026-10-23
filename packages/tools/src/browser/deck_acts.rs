//! What is done to the window's tabs by their keys, the person's clicks on the strip and the
//! agent's calls alike, and the tabs as both read them.

use super::super::host::{Listed, Shared};
use super::super::strip::{Click, Key};
use super::{Deck, Shown};

/// What became of the window.
#[derive(Debug, PartialEq, Eq)]
pub(in super::super) enum After {
    Kept,
    /// The browser's last tab closed; `empty` once nothing is left in the window.
    BrowserGone {
        empty: bool,
    },
    Empty,
}

impl Deck {
    /// Show or close the tab `click` names; why not, when it is not there.
    pub fn act(&mut self, click: Click, shared: &Shared) -> Result<After, String> {
        let key = match click {
            Click::Show(key) | Click::Close(key) => key,
        };
        if !self.listing(shared).iter().any(|tab| tab.id == key.name()) {
            let ids: Vec<String> = self.listing(shared).into_iter().map(|tab| tab.id).collect();
            return Err(format!(
                "there is no tab {}: the tabs are {}",
                key.name(),
                ids.join(", ")
            ));
        }
        Ok(match click {
            Click::Show(Key::Web(id)) => {
                if let Some(web) = self.web.as_mut() {
                    web.switch(id, shared)?;
                }
                self.show(Shown::Web);
                After::Kept
            }
            Click::Show(Key::Term(id)) => {
                self.show(Shown::Term(id));
                After::Kept
            }
            Click::Close(Key::Term(id)) => match self.remove_term(id) {
                true => After::Kept,
                false => After::Empty,
            },
            Click::Close(Key::Web(id)) => {
                let left = self.web.as_mut().map(|web| web.close(id, shared));
                match left {
                    Some(Ok(false)) => {
                        shared.closed();
                        After::BrowserGone {
                            empty: !self.remove_web(),
                        }
                    }
                    Some(Err(why)) => return Err(why),
                    _ => After::Kept,
                }
            }
        })
    }

    /// Every tab, the browser's first: its key, its kind, its title and what it holds.
    pub fn listing(&self, shared: &Shared) -> Vec<Listed> {
        let mut out = vec![];
        if let Some(web) = &self.web {
            let (ids, active) = web.listed();
            let known = shared.tabs.lock().unwrap().clone();
            for id in ids {
                let tab = known.iter().find(|tab| tab.id == id);
                let (title, url) = tab.map_or((String::new(), String::new()), |tab| {
                    (tab.title.trim().to_string(), tab.url.clone())
                });
                out.push(Listed {
                    id: Key::Web(id).name(),
                    kind: "browser",
                    title: if title.is_empty() { url.clone() } else { title },
                    detail: url,
                    shown: self.shown == Shown::Web && id == active,
                    acted_on: id == active,
                });
            }
        }
        for open in &self.terms {
            let title = open.title.strip_prefix("llx · ").unwrap_or(&open.title);
            out.push(Listed {
                id: Key::Term(open.id).name(),
                kind: "terminal",
                title: title.to_string(),
                detail: String::new(),
                shown: self.shown == Shown::Term(open.id),
                acted_on: false,
            });
        }
        out
    }
}
