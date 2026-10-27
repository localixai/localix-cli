//! The window's layout: the strip along the top, what is shown under it, the rest beside, out of
//! sight, and the strip told what the window holds. While the history menu is open it takes the
//! whole window.

use std::sync::mpsc::SyncSender;

use tao::dpi::LogicalSize;

use super::super::capture::{Shot, capture};
use super::super::tabs::Tabs;

use super::super::host::Shared;
use std::sync::atomic::Ordering;

use super::super::strip::{self, HEIGHT, Item, Key, ROW};
use super::super::tabs::{pin, place};
use super::{Deck, Shown};

impl Deck {
    /// What the mini shows: the tab shown, and which of the browser's tabs it is.
    pub(in super::super) fn signature(&self) -> (Shown, u32) {
        (
            self.shown,
            self.web.as_ref().map_or(0, |web| web.listed().1),
        )
    }

    /// The tab shown, as the mini names it: its key and its project.
    pub(in super::super) fn shown_tab(&self, shared: &Shared) -> Option<(String, String)> {
        let listed = self.listing(shared);
        let tab = listed.into_iter().find(|tab| tab.shown)?;
        Some((tab.id, tab.group))
    }

    /// A picture of the tab shown, for the mini; the reply says why when there is none.
    pub(in super::super) fn capture_shown(&self, reply: SyncSender<Result<Shot, String>>) {
        let view = match self.shown {
            Shown::Web => self.web.as_ref().map(Tabs::active),
            Shown::Term(id) => self.term(id).map(|open| &open.webview),
        };
        match view {
            Some(view) => capture(view, None, reply),
            None => {
                let _ = reply.send(Err("no tab is shown".into()));
            }
        }
    }

    /// How tall the strip is: its tabs, and above them the projects when there are several.
    fn top(&self) -> f64 {
        match (self.strip.is_some(), self.rows.load(Ordering::SeqCst)) {
            (false, _) => 0.0,
            (true, false) => HEIGHT,
            (true, true) => HEIGHT + ROW,
        }
    }

    /// The page's part of the window: under the strip.
    pub(super) fn content(&self) -> (LogicalSize<f64>, f64) {
        let native = &self.frame.native;
        let size = native.inner_size().to_logical::<f64>(native.scale_factor());
        let top = self.top();
        (
            LogicalSize::new(size.width, (size.height - top).max(0.0)),
            top,
        )
    }

    /// Give the pages `width` by `height` (the browser's viewport), the strip above them.
    pub(in super::super) fn resize(&self, width: f64, height: f64) {
        let top = self.top();
        self.frame
            .native
            .set_inner_size(LogicalSize::new(width, height + top));
    }

    /// The strip along the top, what is shown under it, and the rest beside, out of sight.
    pub(in super::super) fn layout(&self) {
        let (size, top) = self.content();
        if let Some(strip) = &self.strip {
            // The open menu has the whole window: the pages stand aside, still rendering.
            let height = if self.menu { top + size.height } else { top };
            pin(strip, LogicalSize::new(size.width, height));
        }
        if let Some(web) = &self.web {
            web.layout(size, top, self.shown == Shown::Web && !self.menu);
        }
        for open in &self.terms {
            let here = self.shown == Shown::Term(open.id) && !self.menu;
            let x = if here { 0.0 } else { -(size.width + 100.0) };
            place(&open.webview, x, top, size, here);
        }
    }

    /// Lay the window out again and tell the strip what it holds now.
    pub(in super::super) fn refresh(&self, shared: &Shared) {
        let listed = self.listing(shared);
        let several =
            (listed.first()).is_some_and(|first| listed.iter().any(|tab| tab.group != first.group));
        self.rows.store(several, Ordering::SeqCst);
        self.layout();
        // Again on every refresh: a tab switched to, or a page loaded, comments as the strip says.
        if let Some(web) = &self.web {
            let script = strip::comment(self.commenting, self.sent);
            let _ = web.active().evaluate_script(&script);
        }
        let Some(strip) = &self.strip else {
            return;
        };
        let shown = listed
            .iter()
            .find(|tab| tab.shown)
            .map(|tab| tab.id.clone());
        let items: Vec<Item> = listed
            .into_iter()
            .filter_map(|tab| {
                Some(Item {
                    key: Key::parse(&tab.id)?,
                    title: tab.title,
                    group: tab.group,
                })
            })
            .collect();
        let shown = shown.and_then(|id| Key::parse(&id)).unwrap_or(Key::Web(0));
        let _ = strip.evaluate_script(&strip::script(&items, shown));
        let rows = self.menu.then(|| self.rows(shared));
        let _ = strip.evaluate_script(&strip::menu(rows.as_deref()));
        let held = shared.comments.count();
        let _ = strip.evaluate_script(&strip::commenting(self.commenting, held));
    }
}
