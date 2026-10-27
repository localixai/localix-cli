//! What is done to the window's tabs by their keys, the person's clicks on the strip and the
//! agent's calls alike, and the tabs as both read them.

use wry::WebView;

use super::super::host::{Heard, STRIP, Shared};
use super::super::panel::Held;
use super::super::strip::{self, Click, Key};
use super::{Deck, Shown};
use crate::window::WindowTab;

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
    pub(crate) fn act(&mut self, click: Click, shared: &Shared) -> Result<After, String> {
        let key = match click {
            Click::Show(key) | Click::Close(key) => key,
            Click::History => {
                self.menu = !self.menu;
                return Ok(After::Kept);
            }
            Click::Back | Click::Forward => return Ok(self.walk(click)),
            // The host opens its page: the tab it needs is built on the event loop.
            Click::Visit(_) => return Ok(After::Kept),
            Click::Drag => {
                let _ = self.frame.native.drag_window();
                return Ok(After::Kept);
            }
            Click::Zoom => {
                let native = &self.frame.native;
                native.set_maximized(!native.is_maximized());
                return Ok(After::Kept);
            }
            Click::Send => {
                for (tab, said) in shared.comments.take() {
                    shared.heard(&tab, Heard::Sent(&said));
                }
                self.sent = self.sent.wrapping_add(1);
                return Ok(After::Kept);
            }
            Click::Comment => {
                self.commenting = !self.commenting;
                self.menu = false;
                return Ok(After::Kept);
            }
        };
        self.menu = false;
        // The agent shows a terminal the person closed: a tab again.
        if let Click::Show(Key::Term(id)) = click {
            self.unhide_term(id);
        }
        if !self.listing(shared).iter().any(|tab| tab.id == key.name()) {
            let ids: Vec<String> = self.listing(shared).into_iter().map(|tab| tab.id).collect();
            return Err(format!(
                "there is no tab {}: the tabs are {}",
                key.name(),
                ids.join(", ")
            ));
        }
        Ok(match click {
            // Answered above.
            Click::Back
            | Click::Forward
            | Click::History
            | Click::Visit(_)
            | Click::Comment
            | Click::Send
            | Click::Drag
            | Click::Zoom => After::Kept,
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
            Click::Close(Key::Term(id)) => match self.hide_term(id) {
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

    /// Back or forward in the page shown, when a page is.
    fn walk(&mut self, click: Click) -> After {
        self.menu = false;
        if let (Some(web), Shown::Web) = (&self.web, self.shown) {
            let script = if click == Click::Back {
                "history.back()"
            } else {
                "history.forward()"
            };
            let _ = web.active().evaluate_script(script);
        }
        After::Kept
    }

    /// Panel `id`'s page, or the strip's (`STRIP`).
    pub(crate) fn page(&self, id: u32) -> Option<&WebView> {
        match id {
            STRIP => self.strip.as_ref(),
            id => self.term(id).map(|open| &open.webview),
        }
    }

    /// Every tab as the strip shows them: a project's together, the browser's first in each; its
    /// key, its kind, its title, what it holds and its group.
    pub(crate) fn listing(&self, shared: &Shared) -> Vec<WindowTab> {
        let groups = crate::lock(&shared.groups).clone();
        let group = |key: Key| groups.get(&key.name()).cloned().unwrap_or_default();
        let mut out = vec![];
        if let Some(web) = &self.web {
            let (ids, active) = web.listed();
            let known = crate::lock(&shared.tabs).clone();
            for id in ids {
                let tab = known.iter().find(|tab| tab.id == id);
                let (title, url) = tab.map_or((String::new(), String::new()), |tab| {
                    (tab.title.trim().to_string(), tab.url.clone())
                });
                out.push(WindowTab {
                    id: Key::Web(id).name(),
                    kind: "browser".into(),
                    title: if title.is_empty() { url.clone() } else { title },
                    detail: url,
                    shown: self.shown == Shown::Web && id == active,
                    acted_on: id == active,
                    group: group(Key::Web(id)),
                });
            }
        }
        for open in self.terms.iter().filter(|open| open.held == Held::Live) {
            let title = open.title.strip_prefix("llx · ").unwrap_or(&open.title);
            out.push(WindowTab {
                id: Key::Term(open.id).name(),
                kind: "terminal".into(),
                title: title.to_string(),
                detail: String::new(),
                shown: self.shown == Shown::Term(open.id),
                acted_on: false,
                group: group(Key::Term(open.id)),
            });
        }
        strip::by_group(out, |tab| tab.group.as_str())
    }
}
