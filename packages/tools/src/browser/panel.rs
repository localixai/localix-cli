//! Windows beside the browser's that show what a caller paints (a terminal session's screen).
//! Each is its own native window on the same event loop; an event names the window it is for.
//! A panel shows the page its caller gives and runs the frames painted into it: it takes no
//! input (no IPC), never navigates past its own page, and never takes the keyboard.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use wry::{NewWindowResponse, PageLoadEvent, WebView, WebViewBuilder};

use super::frame::{self, Frame, Look, Rgb, fill};
use super::host::Cmd;
use super::tab_view::build;

/// A panel's size in points: a 120 by 40 grid at a readable size.
pub(super) const WIDTH: f64 = 992.0;
const HEIGHT: f64 = 728.0;
/// How far each panel opens from the last, and from the screen's edges.
const STEP: f64 = 28.0;
const MARGIN: f64 = 16.0;
const TOP: f64 = 60.0;

/// What a sender does with a command: false once the loop is gone.
pub(super) type Post = Box<dyn Fn(Cmd) -> bool + Send>;

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
    pub fn latest(&self) -> Option<String> {
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

/// An open panel, as the loop keeps it. The page goes before its window.
pub(super) struct Open {
    pub id: u32,
    pub webview: WebView,
    pub frame: Frame,
    pub slot: Arc<Slot>,
}

/// What a panel's page is asked for the size it wants: its grid at a comfortable font.
pub(super) const SHAPE: &str = "typeof shape === 'function' ? shape() : null";

impl Open {
    /// The page fills the window again, at its new size or scale.
    pub fn layout(&self) {
        let native = &self.frame.native;
        let size = native.inner_size().to_logical(native.scale_factor());
        let _ = self.webview.set_bounds(fill(size, 0.0));
    }

    /// The window takes the size its page asked for, and keeps that shape.
    pub fn reshape(&self, (width, height): (f64, f64)) {
        self.frame.reshape(LogicalSize::new(width, height));
        self.layout();
    }
}

/// What a panel opens with: its id, title, page, ground (the page's own colour, if it has one),
/// and what it shares with its caller.
pub(super) type Opening = (u32, String, String, Option<Rgb>, Arc<Slot>);

/// Open a panel at the screen's right edge.
pub(super) fn open(
    (id, title, html, ground, slot): Opening,
    target: &EventLoopWindowTarget<Cmd>,
    proxy: EventLoopProxy<Cmd>,
) -> Result<Open, String> {
    let at = target.primary_monitor().map(|monitor| {
        let scale = monitor.scale_factor();
        let at = monitor.position().to_logical::<f64>(scale);
        let size = monitor.size().to_logical::<f64>(scale);
        let (x, y) = place((at.x, at.y, size.width, size.height), id);
        LogicalPosition::new(x, y)
    });
    let size = LogicalSize::new(WIDTH, HEIGHT);
    let look = frame::look(false, frame::glass_allowed());
    let mut frame = Frame::build(target, (&title, size, at), look, Some(ground))?;
    let titled = Rc::downgrade(&frame.native);
    let first = AtomicBool::new(true);
    let webview = WebViewBuilder::new()
        .with_html(frame::dress(&html, look))
        // Over glass, the page is clear: the window's material and coat are its ground.
        .with_transparent(look == Look::Glass)
        .with_bounds(fill(size, 0.0))
        .with_incognito(true)
        .with_focused(false)
        // It draws while the person works elsewhere: its app is never the active one.
        .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled)
        // Its own page only: whatever the page is told, it goes nowhere else.
        .with_navigation_handler(move |_| first.swap(false, Ordering::SeqCst))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_document_title_changed_handler(move |title| {
            if let Some(native) = titled.upgrade() {
                native.set_title(&title);
            }
        })
        // A frame painted before the page could run it is painted again once it can.
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = proxy.send_event(Cmd::Paint(id));
                let _ = proxy.send_event(Cmd::Shape(id));
            }
        });
    // A child of the window's own view, as a tab is: the frame keeps its view for the glass.
    let webview = build(webview, &frame.native).map_err(|error| error.to_string())?;
    frame.show(&webview);
    Ok(Open {
        id,
        webview,
        frame,
        slot,
    })
}

/// Where panel `id` opens on `screen` (x, y, width, height): at its right edge, each panel a step
/// down and left of the last, every fourth back where the first was.
pub(super) fn place((x, y, width, _): (f64, f64, f64, f64), id: u32) -> (f64, f64) {
    let step = STEP * f64::from(id % 4);
    let left = (x + width - WIDTH - MARGIN - step).max(x);
    (left, y + TOP + step)
}

/// Whose window an event is for.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Route {
    /// The panel at this index.
    Panel(usize),
    Browser,
    /// A window already gone, or none of ours.
    Other,
}

/// Route an event for window `id`: the browser's window is `browser` while it is open.
pub(super) fn route<T: PartialEq>(id: T, browser: Option<T>, panels: &[T]) -> Route {
    match panels.iter().position(|panel| *panel == id) {
        Some(index) => Route::Panel(index),
        None if browser == Some(id) => Route::Browser,
        None => Route::Other,
    }
}

#[cfg(test)]
#[path = "panel_tests.rs"]
mod tests;
