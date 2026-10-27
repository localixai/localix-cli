//! What the work thread asks of the window through [`Browser`]: open it, add a panel, list,
//! show, close and size its tabs, and hear what the person does in it.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use contracts::view::{ViewError, ViewResult};
use serde_json::Value;

use crate::window::WindowTab;

use super::super::panel::Panel;
use super::super::strip::{Click, Key};
use super::super::webview::WebviewDriver;
use super::{Browser, Cmd, Dressing, Heard, STRIP, Window};

impl Browser {
    /// Open the window on `window.url` and wait for its document. One window per process: the
    /// browser shows one page.
    pub fn open(&self, window: Window) -> ViewResult<WebviewDriver> {
        let (reply, opened) = mpsc::sync_channel(1);
        self.proxy
            .send_event(Cmd::Open { window, reply })
            .map_err(|error| {
                ViewError::Unavailable(format!("the browser host is gone: {error}"))
            })?;
        opened
            .recv()
            .map_err(|error| ViewError::Unavailable(format!("the browser host is gone: {error}")))?
            .map_err(|error| {
                ViewError::Unavailable(format!("could not open the browser window: {error}"))
            })?;
        Ok(WebviewDriver::new(
            self.proxy.clone(),
            Arc::clone(&self.shared),
        ))
    }

    /// A driver for the window as it is open now, for a client that did not open it (the daemon
    /// has one window, and every client of it works in tabs of its own); `None` while it is not.
    pub fn attach(&self) -> Option<WebviewDriver> {
        self.shared
            .open
            .load(Ordering::SeqCst)
            .then(|| WebviewDriver::new(self.proxy.clone(), Arc::clone(&self.shared)))
    }

    /// A tab `id` (unique among the panels: a terminal session's id) titled `title` in the window, showing `html` (a page of the caller's, which paints
    /// run frames in), shown once it opens; it closes when the panel is dropped. It opens the
    /// window when the browser has not.
    pub fn panel(&self, id: u32, title: &str, html: String, ground: Option<[u8; 3]>) -> Panel {
        let proxy = self.proxy.clone();
        let panel = Panel::new(id, Box::new(move |cmd| proxy.send_event(cmd).is_ok()));
        let slot = Arc::clone(&panel.slot);
        let open = Cmd::Panel((id, title.to_string(), html, ground, slot));
        if self.proxy.send_event(open).is_err() {
            panel.slot.alive.store(false, Ordering::SeqCst);
        }
        panel
    }

    /// For checks: how the window is dressed: its materials, the first one's state, whether
    /// the window is opaque, the shape its resizing keeps (width over height, 0 for none),
    /// whether it is coated. `None` while it is closed.
    #[doc(hidden)]
    pub fn dressed(&self) -> Option<Dressing> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.proxy.send_event(Cmd::Dressed(reply)).ok()?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()?
    }

    /// Hear what the person does in the window: a tab or the window closed, by key (`window` for
    /// the whole of it) and in words. It runs on the window's thread: it must not wait.
    pub fn listen(&self, listener: impl Fn(&str, Heard<'_>) + Send + Sync + 'static) {
        *crate::lock(&self.shared.listener) = Some(Arc::new(listener));
    }

    /// The window's tabs, in the strip's order; `None` while it is closed.
    pub fn listing(&self) -> Option<Vec<WindowTab>> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.proxy.send_event(Cmd::Listing(reply)).ok()?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()?
    }

    /// Size the whole window to `width` by `height` points, as the person would.
    pub fn resize_window(&self, width: f64, height: f64) -> Result<(), String> {
        let (reply, answer) = mpsc::sync_channel(1);
        let ask = Cmd::Sized(width, height, reply);
        self.proxy.send_event(ask).map_err(gone)?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(gone)?
    }

    /// Show the tab `id` (`w1`, `t2`), or take it out of the window (`close`).
    pub fn act(&self, id: &str, close: bool) -> Result<(), String> {
        let key = Key::parse(id).ok_or_else(|| format!("there is no tab {id}"))?;
        let click = if close {
            Click::Close(key)
        } else {
            Click::Show(key)
        };
        let (reply, answer) = mpsc::sync_channel(1);
        self.proxy
            .send_event(Cmd::Act(click, reply))
            .map_err(gone)?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(gone)?
    }

    /// For checks: what the person's click on the strip says (`show t1`, `close w2`).
    #[doc(hidden)]
    pub fn click(&self, said: &str) {
        let _ = self.proxy.send_event(Cmd::Clicked(said.to_string()));
    }

    /// Tab `key` (`w1`, `t2`) is of `group`, a project: the strip draws each one's tabs
    /// together once there are several.
    pub fn group(&self, key: &str, group: &str) {
        crate::lock(&self.shared.groups).insert(key.to_string(), group.to_string());
        let _ = self.proxy.send_event(Cmd::Strip);
    }

    /// What `script` evaluates to in `panel`'s page, as JSON.
    pub fn peek(&self, panel: &Panel, script: &str) -> Result<Value, String> {
        self.peek_in(panel.id(), script)
    }

    /// For checks: what `script` evaluates to in the strip's page.
    #[doc(hidden)]
    pub fn peek_strip(&self, script: &str) -> Result<Value, String> {
        self.peek_in(STRIP, script)
    }

    fn peek_in(&self, panel: u32, script: &str) -> Result<Value, String> {
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::sync_channel(1);
        crate::lock(&self.shared.pending).insert(id, sender);
        let script = script.to_string();
        let panel = Some(panel);
        let _ = (self.proxy).send_event(Cmd::Eval {
            id,
            script,
            failed: "",
            panel,
            tab: None,
        });
        let reply = receiver.recv_timeout(std::time::Duration::from_secs(5));
        crate::lock(&self.shared.pending).remove(&id);
        reply.unwrap_or_else(|error| Err(format!("no answer: {error}")))
    }
}

/// Why the window could not be asked: it is not open, and how that showed.
fn gone(error: impl std::fmt::Display) -> String {
    format!("the window is not open: {error}")
}
