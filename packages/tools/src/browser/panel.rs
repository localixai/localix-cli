//! Tabs in the one window (deck.rs) that show what a caller paints: a terminal session's screen.
//! A panel shows the page its caller gives and runs the frames painted into it: it takes no
//! input (no IPC), never navigates past its own page, and never takes the keyboard.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use tao::event_loop::EventLoopProxy;
use wry::{NewWindowResponse, PageLoadEvent, WebView, WebViewBuilder};

use super::frame::{self, Look, Rgb};
use super::host::Cmd;
use super::tab_view::build;

/// What a sender does with a command: false once the loop is gone. Shared, as a painter is
/// (the loop's proxy is).
pub(super) type Post = Box<dyn Fn(Cmd) -> bool + Send + Sync>;

/// What a panel and the loop share: the newest frame, whether a paint is queued, and whether the
/// window is still open.
#[derive(Default)]
pub(super) struct Slot {
    frame: Mutex<Option<String>>,
    queued: AtomicBool,
    pub alive: AtomicBool,
}

impl Slot {
    /// The newest frame, for the loop: a paint after this one queues again. The frame stays, for
    /// a page that loads after it was painted.
    pub(super) fn latest(&self) -> Option<String> {
        self.queued.store(false, Ordering::SeqCst);
        let frame = self.frame.lock().unwrap_or_else(PoisonError::into_inner);
        frame.clone()
    }
}

/// A panel as its caller holds it: painted from any thread, closed when dropped.
pub struct Panel {
    id: u32,
    pub(super) slot: Arc<Slot>,
    send: Post,
}

impl Panel {
    pub(super) fn new(id: u32, send: Post) -> Self {
        let slot = Slot {
            alive: AtomicBool::new(true),
            ..Slot::default()
        };
        Self {
            id,
            slot: Arc::new(slot),
            send,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    /// Show `frame`, a script the page runs. At most one paint waits in the loop's queue, and it
    /// shows the newest frame. False once the window is closed.
    pub fn paint(&self, frame: String) -> bool {
        *self
            .slot
            .frame
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(frame);
        if !self.slot.queued.swap(true, Ordering::SeqCst) && !(self.send)(Cmd::Paint(self.id)) {
            self.slot.alive.store(false, Ordering::SeqCst);
        }
        self.slot.alive.load(Ordering::SeqCst)
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        (self.send)(Cmd::Unpanel(self.id));
    }
}

/// An open panel, as the loop keeps it: a tab of the window, titled as its page says.
pub(super) struct Open {
    pub id: u32,
    pub title: String,
    pub webview: WebView,
    pub slot: Arc<Slot>,
}

/// What a panel opens with: its id, title, page, ground (the page's own colour, if it has one),
/// and what it shares with its caller.
pub(super) type Opening = (u32, String, String, Option<Rgb>, Arc<Slot>);

/// Open a panel as a tab of `native`, the window, in its `look`.
pub(super) fn open(
    (id, title, html, _, slot): Opening,
    native: &tao::window::Window,
    look: Look,
    proxy: EventLoopProxy<Cmd>,
) -> Result<Open, String> {
    let first = AtomicBool::new(true);
    let titles = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_html(frame::dress(&html, look))
        // Over glass, the page is clear: the window's material and coat are its ground.
        .with_transparent(look == Look::Glass)
        .with_incognito(true)
        .with_focused(false)
        // It draws while the person works elsewhere: its app is never the active one.
        .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled)
        // Its own page only: whatever the page is told, it goes nowhere else.
        .with_navigation_handler(move |_| first.swap(false, Ordering::SeqCst))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_document_title_changed_handler(move |title| {
            let _ = titles.send_event(Cmd::Titled(id, title));
        })
        // A frame painted before the page could run it is painted again once it can.
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = proxy.send_event(Cmd::Paint(id));
            }
        });
    let webview = build(webview, native).map_err(|error| error.to_string())?;
    super::platform::keep_tab_rendering(&webview);
    Ok(Open {
        id,
        title,
        webview,
        slot,
    })
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
