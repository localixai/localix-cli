//! The worker's side of the native window: each [`Driver`] call becomes a probe call on the page
//! (webview_driver.rs); here, the calls themselves and the window's tabs.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use contracts::view::{ViewError, ViewResult};
use serde_json::Value;
use tao::event_loop::EventLoopProxy;

use super::Driver;
use super::host::{Cmd, Loads, Reply, Shared, Tab, WINDOW_CLOSED, active_loads};

#[path = "webview_driver.rs"]
mod driver;

const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a page that is replacing itself may take before the probe answers again.
const PROBE_READY: Duration = Duration::from_secs(5);
const LOAD_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone)]
pub struct WebviewDriver {
    proxy: EventLoopProxy<Cmd>,
    shared: Arc<Shared>,
    /// The tab it acts in, whatever tab is shown; else the one acted on.
    tab: Option<u32>,
}

impl WebviewDriver {
    pub(super) fn new(proxy: EventLoopProxy<Cmd>, shared: Arc<Shared>) -> Self {
        Self {
            proxy,
            shared,
            tab: None,
        }
    }

    /// The same window, acting in tab `id` whatever tab is shown or acted on: tabs pinned so
    /// work at the same time.
    #[must_use]
    pub fn on(&self, id: u32) -> Self {
        Self {
            tab: Some(id),
            ..self.clone()
        }
    }

    /// The tab its calls go to.
    fn target(&self) -> u32 {
        self.tab
            .unwrap_or_else(|| self.shared.active.load(Ordering::SeqCst))
    }

    /// Whether the window is open: the person may have closed it.
    pub fn is_open(&self) -> bool {
        self.shared.open.load(Ordering::SeqCst)
    }

    /// The window is open, and so is the tab it is pinned to: a pinned call never goes to the tab
    /// shown instead, where a goal's clicks would land on another page.
    fn open_or_closed(&self) -> ViewResult<()> {
        if !self.is_open() {
            return Err(closed());
        }
        match self.tab {
            Some(id)
                if !crate::lock(&self.shared.tabs)
                    .iter()
                    .any(|tab| tab.id == id) =>
            {
                Err(ViewError::Unavailable(format!(
                    "tab {id} is closed: list shows the tabs open"
                )))
            }
            Some(_) | None => Ok(()),
        }
    }

    /// Send the script `form` makes of a fresh call id, its answer awaited on the receiver; the
    /// caller removes the id from `pending` once it is done waiting.
    fn post(
        &self,
        form: impl FnOnce(u64) -> String,
        failed: &'static str,
    ) -> ViewResult<(u64, mpsc::Receiver<Reply>)> {
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::sync_channel(1);
        crate::lock(&self.shared.pending).insert(id, sender);
        let script = form(id);
        let sent = self.send(Cmd::Eval {
            id,
            script,
            failed,
            panel: None,
            tab: self.tab,
        });
        if sent.is_err() {
            crate::lock(&self.shared.pending).remove(&id);
        }
        sent.map(|()| (id, receiver))
    }

    /// One probe call, answered over IPC or refused while the page has no probe.
    fn call(&self, name: &'static str, args: &Value, timeout: Duration) -> ViewResult<Value> {
        self.open_or_closed()?;
        let call = |id| {
            format!("window.__dloop_call ? window.__dloop_call({id}, {name:?}, {args}) : false")
        };
        let (id, receiver) = self.post(call, "probe_not_ready")?;
        let reply = receiver.recv_timeout(timeout);
        crate::lock(&self.shared.pending).remove(&id);
        match reply {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(classify(&error)),
            Err(_) => Err(ViewError::Unavailable(format!(
                "the page did not answer {name}"
            ))),
        }
    }

    /// Retry `name` while the page is between documents (no probe yet, or no body).
    #[expect(
        clippy::disallowed_methods,
        reason = "a backoff: the page between documents says nothing when its probe is ready"
    )]
    fn call_ready(&self, name: &'static str, args: &Value) -> ViewResult<Value> {
        let deadline = Instant::now() + PROBE_READY;
        loop {
            match self.call(name, args, CALL_TIMEOUT) {
                Ok(Value::Null) | Err(ViewError::Unavailable(_))
                    if Instant::now() < deadline && self.is_open() =>
                {
                    std::thread::sleep(Duration::from_millis(30));
                }
                Ok(Value::Null) => {
                    return Err(ViewError::Unavailable("the page has no document".into()));
                }
                other => return other,
            }
        }
    }

    /// The loads of the tab acted on.
    fn loads(&self) -> Loads {
        active_loads(&crate::lock(&self.shared.loads), self.target())
    }

    /// Wait for a load newer than `before` to start, up to `grace`: in the tab acted on, which
    /// may be a tab the page just opened.
    fn load_started_since(&self, before: Loads, grace: Duration) -> bool {
        let guard = crate::lock(&self.shared.loads);
        let active = || self.target();
        let (loads, _) = self
            .shared
            .loaded
            .wait_timeout_while(guard, grace, |loads| {
                active_loads(loads, active()).started == before.started
            })
            .unwrap_or_else(PoisonError::into_inner);
        active_loads(&loads, active()).started != before.started
    }

    /// The tabs open now, and the id of the one acted on (this driver's own, when pinned).
    pub fn tabs(&self) -> (Vec<Tab>, u32) {
        let tabs = crate::lock(&self.shared.tabs).clone();
        (tabs, self.target())
    }

    /// Open `url` in a new tab, show it and act on it from now on; then wait for its document.
    pub fn new_tab(&self, url: &str) -> ViewResult<u32> {
        let id = self.add_tab(url)?;
        self.on(id).wait_new();
        Ok(id)
    }

    /// Open `url` in a new tab, shown, and say its id at once: its document is waited for by a
    /// driver pinned to it (`wait_new`), so several tabs load at the same time.
    pub fn add_tab(&self, url: &str) -> ViewResult<u32> {
        let url = loadable(url)?;
        self.open_or_closed()?;
        let (reply, opened) = mpsc::sync_channel(1);
        self.send(Cmd::NewTab {
            url,
            reply: Some(reply),
        })?;
        answered(&opened)
    }

    /// Wait for the first document of a tab just opened.
    pub fn wait_new(&self) {
        self.load_started_since(Loads::default(), PROBE_READY);
        let _ = self.wait_loaded(LOAD_TIMEOUT);
    }

    /// Show tab `id` and act on it from now on.
    pub fn switch_tab(&self, id: u32) -> ViewResult<()> {
        self.open_or_closed()?;
        let (reply, switched) = mpsc::sync_channel(1);
        self.send(Cmd::SwitchTab { id, reply })?;
        answered(&switched)
    }

    /// Close tab `id`; returns whether tabs are left (with none, the window is closed).
    pub fn close_tab(&self, id: u32) -> ViewResult<bool> {
        self.open_or_closed()?;
        let (reply, closed) = mpsc::sync_channel(1);
        self.send(Cmd::CloseTab { id, reply })?;
        answered(&closed)
    }

    fn send(&self, command: Cmd) -> ViewResult<()> {
        self.proxy
            .send_event(command)
            .map_err(|error| ViewError::Unavailable(format!("the browser window is gone: {error}")))
    }
}

/// What the event loop answered a command with.
/// `url` as the window can load it, else refused before it reaches the window (`guard`).
pub(super) fn loadable(url: &str) -> ViewResult<String> {
    crate::browser::guard::loadable(url)
        .ok_or_else(|| ViewError::Refused(format!("`{url}` is not an address")))
}

fn answered<T>(answer: &mpsc::Receiver<Result<T, String>>) -> ViewResult<T> {
    match answer.recv_timeout(CALL_TIMEOUT) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(why)) => Err(ViewError::Refused(why)),
        Err(_) => Err(closed()),
    }
}

/// Why nothing can be done on the page: the person closed the window.
fn closed() -> ViewError {
    ViewError::Unavailable("the browser window was closed".into())
}

fn classify(error: &str) -> ViewError {
    if error == WINDOW_CLOSED {
        closed()
    } else if error.starts_with("probe_not_ready") {
        ViewError::Unavailable(error.to_string())
    } else if error.starts_with("stale") {
        ViewError::Stale(error.to_string())
    } else {
        ViewError::Refused(error.to_string())
    }
}

impl WebviewDriver {
    /// Close the window; calls waiting on it fail, and the next `Page::open` opens a new one.
    pub fn close(&self) {
        let _ = self.send(Cmd::Close);
    }
}
