//! The native window: a `tao` event loop on the main thread that opens one window when asked (it
//! exists only once it is needed): the browser's tabs and the terminal sessions' screens are its
//! tabs (deck.rs), with a `wry` WebView each. The caller's work runs on a worker thread and
//! reaches the page through [`Cmd`]s on the event loop's proxy; answers come back over the page's
//! IPC channel, matched by call id and a per-run token the page cannot read. Here, what they
//! share; the loop is host_events.rs (the browser's commands host_web.rs), the work thread's
//! handle host_handle.rs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use tao::event_loop::EventLoopProxy;
use wry::PageLoadEvent;

use super::capture::{Clip, Shot};
use super::history::History;
use super::panel;
use super::strip::Click;
use super::viewport::Viewport;

#[path = "host_events.rs"]
mod events;
#[path = "host_handle.rs"]
mod handle;
#[path = "host_web.rs"]
mod web;
pub use events::host;

/// One script from six files: snapshot, same-origin frames, the outline, actions, keys, then
/// evaluate and the IPC entry point.
pub(super) const PROBE: &str = concat!(
    include_str!("probe/snapshot.js"),
    include_str!("probe/frames.js"),
    include_str!("probe/outline.js"),
    include_str!("probe/act.js"),
    include_str!("probe/keys.js"),
    include_str!("probe/evaluate.js"),
);

/// How the window is opened.
#[derive(Debug, Clone)]
pub struct Window {
    pub url: String,
    pub title: String,
    /// Keep the window off screen and out of the Dock; the page still renders.
    pub headless: bool,
    /// The page's size.
    pub viewport: Viewport,
}

pub(super) enum Cmd {
    /// Open the window (once); the reply says whether it opened.
    Open {
        window: Window,
        reply: SyncSender<Result<(), String>>,
    },
    /// A script built by the driver that hands a probe call or its promise to the probe; `false`
    /// or a parse error answers the call at once with `failed` (`probe_not_ready`,
    /// `script_not_started`).
    Eval {
        id: u64,
        script: String,
        failed: &'static str,
        /// In this panel's page instead, answered with the script's value (for checks).
        panel: Option<u32>,
        /// In this browser tab instead of the one acted on.
        tab: Option<u32>,
    },
    /// Load an address: in this tab, else the one acted on.
    Navigate(String, Option<u32>),
    /// Open a tab on `url` and show it. A page's own new window (`target=_blank`, `window.open`)
    /// comes here with no reply.
    NewTab {
        url: String,
        reply: Option<SyncSender<Result<u32, String>>>,
    },
    SwitchTab {
        id: u32,
        reply: SyncSender<Result<(), String>>,
    },
    /// Close a tab; the reply says whether tabs are left (with none, the window closed).
    CloseTab {
        id: u32,
        reply: SyncSender<Result<bool, String>>,
    },
    /// Close the window, as the person would but without stopping the task: it opens again,
    /// shown or hidden, on its next use.
    Close,
    /// Reframe the page (`viewport`): width, height.
    Resize(f64, f64),
    /// A screenshot of the page, or of a part of it; the reply comes once WebKit has drawn it.
    Capture {
        clip: Option<Clip>,
        reply: SyncSender<Result<Shot, String>>,
        tab: Option<u32>,
    },
    /// Open a panel as a tab: its id, title and page, and what it shares with its caller.
    Panel(panel::Opening),
    /// Run a panel's newest frame.
    Paint(u32),
    /// Close a panel.
    Unpanel(u32),
    /// A panel's page took a title.
    Titled(u32, String),
    /// What the person clicked on the strip.
    Clicked(String),
    /// Tell the strip what the window holds (a tab's title changed, its page loaded).
    Strip,
    /// For checks: how the window is dressed, once tinted again.
    Dressed(SyncSender<Option<Dressing>>),
    /// The window's tabs; `None` while it is closed.
    Listing(SyncSender<Option<Vec<Listed>>>),
    /// Size the whole window, in points.
    Sized(f64, f64, SyncSender<Result<(), String>>),
    /// Show or close a tab by its key, for the agent: the task goes on whatever closes.
    Act(Click, SyncSender<Result<(), String>>),
    Exit(i32),
}

pub(super) type Reply = Result<Value, String>;

/// A tab of the window: its key (`w1`, a browser tab; `t2`, a terminal's screen), what kind it
/// is, its title, what it holds (a page's address), whether it is shown, and whether the browser
/// acts on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub id: String,
    pub kind: &'static str,
    pub title: String,
    pub detail: String,
    pub shown: bool,
    pub acted_on: bool,
    /// The project it was opened for, as its opener named it.
    pub group: String,
}

/// How a window is dressed, for checks: see [`Browser::dressed`].
pub type Dressing = (usize, isize, bool, f64, bool);

/// State the event loop and the worker share.
pub(super) struct Shared {
    pub token: String,
    /// Whether the window is open now: the person may close it, and it opens again when needed.
    pub open: AtomicBool,
    pub next_id: AtomicU64,
    pub pending: Mutex<HashMap<u64, SyncSender<Reply>>>,
    /// Each tab's loads, by tab id.
    pub loads: Mutex<HashMap<u32, Loads>>,
    pub loaded: Condvar,
    /// The tab acted on.
    pub active: AtomicU32,
    /// Numbers every load event across the tabs, so a load in another tab never reads as this
    /// tab's (a page that opened a tab has started a load: the new tab's).
    seq: AtomicU64,
    pub tabs: Mutex<Vec<Tab>>,
    /// Who hears what the person does in the window.
    listener: Mutex<Option<Listener>>,
    /// The group (a project) of each tab, by key, as its opener named it.
    pub groups: Mutex<HashMap<String, String>>,
    /// The web pages the tabs have shown, for the strip's history menu.
    pub history: History,
}

/// Told what the person did to a tab (its key; `window` for the whole window), in words.
type Listener = Box<dyn Fn(&str, &str) + Send + Sync>;

/// The loads of tab `active`.
pub(super) fn active_loads(loads: &HashMap<u32, Loads>, active: u32) -> Loads {
    loads.get(&active).copied().unwrap_or_default()
}

/// One tab as the agent reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub id: u32,
    pub url: String,
    pub title: String,
}

/// Page loads started and finished; equal counts mean nothing is loading. `requested` counts the
/// navigations the page asked for: WebKit keeps the old document until a new one commits, so an
/// action is only settled once a navigation it requested has loaded.
#[derive(Default, Clone, Copy)]
pub(super) struct Loads {
    pub requested: u64,
    pub started: u64,
    pub finished: u64,
}

impl Shared {
    /// Tell the listener what the person did to `key`.
    fn heard(&self, key: &str, what: &str) {
        if let Some(listen) = crate::lock(&self.listener).as_ref() {
            listen(key, what);
        }
    }

    fn answer(&self, id: u64, reply: Reply) {
        let waiting = crate::lock(&self.pending).remove(&id);
        if let Some(sender) = waiting {
            let _ = sender.send(reply);
        }
    }

    /// The window was closed: every call waiting on it fails now, rather than at its timeout, and
    /// the load counts start over, so a window opened again waits for its own first document.
    pub(super) fn closed(&self) {
        self.open.store(false, Ordering::SeqCst);
        let waiting = std::mem::take(&mut *crate::lock(&self.pending));
        #[allow(
            clippy::iter_over_hash_type,
            reason = "each waiting call is told on its own channel: the order is no one's"
        )]
        for sender in waiting.into_values() {
            let _ = sender.send(Err(WINDOW_CLOSED.into()));
        }
        crate::lock(&self.loads).clear();
        crate::lock(&self.tabs).clear();
        self.loaded.notify_all();
    }

    pub(super) fn navigation_requested(&self, tab: u32) {
        let next = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        crate::lock(&self.loads).entry(tab).or_default().requested = next;
        self.loaded.notify_all();
    }

    pub(super) fn page_load(&self, tab: u32, event: PageLoadEvent, url: &str) {
        let next = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut loads = crate::lock(&self.loads);
            let loads = loads.entry(tab).or_default();
            match event {
                PageLoadEvent::Started => loads.started = next,
                PageLoadEvent::Finished => loads.finished = loads.started,
            }
        }
        if let Some(entry) = crate::lock(&self.tabs).iter_mut().find(|t| t.id == tab) {
            entry.url = url.to_string();
        }
        if matches!(event, PageLoadEvent::Finished) {
            self.history.visited(url);
        }
        self.loaded.notify_all();
    }

    pub(super) fn titled(&self, tab: u32, title: String) {
        if let Some(entry) = crate::lock(&self.tabs).iter_mut().find(|t| t.id == tab) {
            self.history.titled(&entry.url, &title);
            entry.title = title;
        }
    }

    /// The tabs open now, `active` acted on; what is known of each is kept.
    pub(super) fn set_tabs(&self, active: u32, ids: &[u32]) {
        let mut tabs = crate::lock(&self.tabs);
        tabs.retain(|tab| ids.contains(&tab.id));
        for id in ids {
            if !tabs.iter().any(|tab| tab.id == *id) {
                tabs.push(Tab {
                    id: *id,
                    url: String::new(),
                    title: String::new(),
                });
            }
        }
        tabs.sort_by_key(|tab| tab.id);
        self.active.store(active, Ordering::SeqCst);
        self.loaded.notify_all();
    }

    /// A message from the page: `{t, id, ok}` or `{t, id, err}`. Anything else is ignored.
    pub(super) fn ipc(&self, body: &str) {
        let Ok(message) = serde_json::from_str::<Value>(body) else {
            return;
        };
        if message.get("t").and_then(Value::as_str) != Some(self.token.as_str()) {
            return;
        }
        let Some(id) = message.get("id").and_then(Value::as_u64) else {
            return;
        };
        let reply = match message.get("err").and_then(Value::as_str) {
            Some(error) => Err(error.to_string()),
            None => Ok(message.get("ok").cloned().unwrap_or(Value::Null)),
        };
        self.answer(id, reply);
    }
}

/// The browser host as the work thread holds it: it opens the one window on demand.
#[derive(Clone)]
pub struct Browser {
    proxy: EventLoopProxy<Cmd>,
    shared: Arc<Shared>,
}

/// The panel id that names the strip's own page, for checks.
pub(super) const STRIP: u32 = u32::MAX;

/// The answer to every call while the window is closed.
pub(super) const WINDOW_CLOSED: &str = "window_closed";
