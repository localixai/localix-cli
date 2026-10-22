//! The window's tabs: one native window, one WebView per tab, the shown tab filling the window and
//! the others beside it, out of sight but still rendering (WebKit stops a hidden view's animation
//! frames; a tab moved aside is not hidden). A closed tab's WebView is kept, blank, and reused by
//! the next tab: on macOS the tabs share one WebKit configuration (cookies, the probe's IPC), and
//! the first WebView owns its IPC handler, so none is ever dropped before the window is.

use std::sync::Arc;

use tao::dpi::LogicalSize;
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::WindowBuilder;
use wry::WebView;

use super::host::{Cmd, Shared, Window};
use super::platform::{conceal, keep_rendering, keep_tab_rendering};
use super::tab_view::{SHARED, build, builder, share};

/// Tabs open at once: each is a WebView with its own web process.
pub(super) const MAX_TABS: usize = 8;
pub(super) const BLANK: &str = "about:blank";

/// One WebView: a tab while `open`, else blank and waiting to be reused. Its id is its place + 1.
struct Slot {
    webview: WebView,
    open: bool,
}

pub(super) struct Tabs {
    pub native: tao::window::Window,
    slots: Vec<Slot>,
    /// The id of the tab shown and acted on.
    active: u32,
    window: Window,
}

impl Tabs {
    /// The window, with its first tab on `window.url`, shown without taking focus.
    pub fn open(
        window: &Window,
        target: &EventLoopWindowTarget<Cmd>,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let mut builder = WindowBuilder::new()
            .with_title(&window.title)
            .with_inner_size(LogicalSize::new(
                window.viewport.width,
                window.viewport.height,
            ))
            .with_focused(false);
        if window.headless {
            // Shown only once concealed, so it never flashes on screen.
            builder = conceal(builder.with_decorations(false).with_visible(false));
        }
        let native = builder.build(target).map_err(|error| error.to_string())?;
        let mut tabs = Self {
            native,
            slots: vec![],
            active: 1,
            window: window.clone(),
        };
        let webview = tabs.build(1, &window.url, shared, proxy)?;
        // Shows the window too, without making it the key window (tao's set_visible would).
        keep_rendering(&webview, window.headless);
        #[cfg(not(target_os = "macos"))]
        if window.headless {
            tabs.native.set_visible(true);
        }
        tabs.slots.push(Slot {
            webview,
            open: true,
        });
        shared.set_tabs(1, &tabs.ids());
        tabs.layout();
        Ok(tabs)
    }

    /// The tab shown and acted on.
    pub fn active(&self) -> &WebView {
        &self.slots[self.active as usize - 1].webview
    }

    fn ids(&self) -> Vec<u32> {
        (1..=self.slots.len() as u32)
            .filter(|id| self.slots[*id as usize - 1].open)
            .collect()
    }

    /// Open `url` in a new tab and show it; its id, or why there is none.
    pub fn add(
        &mut self,
        url: &str,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<u32, String> {
        let id = match self.slots.iter().position(|slot| !slot.open) {
            Some(at) => {
                let slot = &mut self.slots[at];
                slot.open = true;
                slot.webview
                    .load_url(url)
                    .map_err(|error| error.to_string())?;
                at as u32 + 1
            }
            None if self.slots.len() < MAX_TABS => {
                let id = self.slots.len() as u32 + 1;
                let webview = self.build(id, url, shared, proxy)?;
                keep_tab_rendering(&webview);
                self.slots.push(Slot {
                    webview,
                    open: true,
                });
                id
            }
            None => return Err(format!("{MAX_TABS} tabs are open already: close one first")),
        };
        self.active = id;
        shared.set_tabs(id, &self.ids());
        self.layout();
        Ok(id)
    }

    /// Show tab `id`.
    pub fn switch(&mut self, id: u32, shared: &Shared) -> Result<(), String> {
        self.check(id)?;
        self.active = id;
        shared.set_tabs(id, &self.ids());
        self.layout();
        Ok(())
    }

    /// Close tab `id`, showing the one before it when it was shown. Returns whether tabs are left:
    /// with none, the window closes.
    pub fn close(&mut self, id: u32, shared: &Shared) -> Result<bool, String> {
        self.check(id)?;
        let slot = &mut self.slots[id as usize - 1];
        slot.open = false;
        let _ = slot.webview.load_url(BLANK);
        let ids = self.ids();
        let Some(last) = ids.last() else {
            return Ok(false);
        };
        if self.active == id {
            self.active = ids
                .iter()
                .rev()
                .find(|other| **other < id)
                .copied()
                .unwrap_or(*last);
        }
        shared.set_tabs(self.active, &ids);
        self.layout();
        Ok(true)
    }

    fn check(&self, id: u32) -> Result<(), String> {
        match self.slots.get((id as usize).wrapping_sub(1)) {
            Some(slot) if slot.open => Ok(()),
            _ => Err(format!("there is no tab {id}: {}", list(&self.ids()))),
        }
    }

    /// The shown tab fills the window; the others stand beside it, out of sight.
    pub fn layout(&self) {
        let size = self
            .native
            .inner_size()
            .to_logical::<f64>(self.native.scale_factor());
        for (at, slot) in self.slots.iter().enumerate() {
            let shown = at as u32 + 1 == self.active;
            let x = if shown { 0.0 } else { -(size.width + 100.0) };
            place(&slot.webview, x, size, shown);
        }
    }

    fn build(
        &self,
        id: u32,
        url: &str,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<WebView, String> {
        let first = self.slots.first().map(|slot| &slot.webview);
        // Where the tabs share a configuration, the first tab's IPC handler serves them all: a
        // second one under the same name raises an Objective-C exception, which a binary built
        // with `panic = "abort"` cannot catch.
        let ipc = !(SHARED && first.is_some());
        let builder = builder(id, url, &self.window, shared, proxy.clone(), ipc);
        let builder = share(builder, first);
        build(builder, &self.native).map_err(|error| error.to_string())
    }
}

/// `ids` as the agent reads them.
fn list(ids: &[u32]) -> String {
    let ids: Vec<String> = ids.iter().map(u32::to_string).collect();
    format!("the tabs are {}", ids.join(", "))
}

/// Where a tab stands. A WebView in a GTK box has no bounds of its own, so there the tabs not
/// shown are hidden instead (WebKitGTK keeps running a hidden view's timers).
#[cfg(not(target_os = "linux"))]
fn place(webview: &WebView, x: f64, size: LogicalSize<f64>, _shown: bool) {
    use wry::dpi::{LogicalPosition, LogicalSize as Size};
    let _ = webview.set_bounds(wry::Rect {
        position: LogicalPosition::new(x, 0.0).into(),
        size: Size::new(size.width, size.height).into(),
    });
}

#[cfg(target_os = "linux")]
fn place(webview: &WebView, _x: f64, _size: LogicalSize<f64>, shown: bool) {
    let _ = webview.set_visible(shown);
}
