//! The native window: a `tao` event loop on the main thread that opens one window when asked (it
//! exists only once it is needed), with a `wry` WebView per tab (tabs.rs). The caller's work runs on a worker thread and
//! reaches the page through [`Cmd`]s on the event loop's proxy; answers come back over the page's
//! IPC channel, matched by call id and a per-run token the page cannot read.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowId;
use wry::PageLoadEvent;

use super::capture::{Clip, Shot, capture};
use super::frame;
use super::guard::token;
use super::panel::{self, Open, Panel, Route, route};
use super::tabs::Tabs;
use super::viewport::Viewport;
use super::webview::WebviewDriver;
use super::{BrowserError, BrowserResult};

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
        reply: Sender<Result<(), String>>,
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
    },
    Navigate(String),
    /// Open a tab on `url` and show it. A page's own new window (`target=_blank`, `window.open`)
    /// comes here with no reply.
    NewTab {
        url: String,
        reply: Option<Sender<Result<u32, String>>>,
    },
    SwitchTab {
        id: u32,
        reply: Sender<Result<(), String>>,
    },
    /// Close a tab; the reply says whether tabs are left (with none, the window closed).
    CloseTab {
        id: u32,
        reply: Sender<Result<bool, String>>,
    },
    /// Close the window, as the person would but without stopping the task: it opens again,
    /// shown or hidden, on its next use.
    Close,
    /// Reframe the page (`viewport`): width, height.
    Resize(f64, f64),
    /// A screenshot of the page, or of a part of it; the reply comes once WebKit has drawn it.
    Capture {
        clip: Option<Clip>,
        reply: Sender<Result<Shot, String>>,
    },
    /// Open a panel: its id, title and page, and what it shares with its caller.
    Panel(panel::Opening),
    /// Run a panel's newest frame.
    Paint(u32),
    /// Close a panel.
    Unpanel(u32),
    /// Ask a panel's page the size it wants (once it loaded)...
    Shape(u32),
    /// ...and give its window that size.
    Reshape(u32, (f64, f64)),
    /// For checks: how a window is dressed, the browser's or a panel's, once tinted again.
    Dressed(Option<u32>, Sender<Option<Dressing>>),
    /// For checks: a panel's window resized.
    PanelSize(u32, f64, f64),
    Exit(i32),
}

pub(super) type Reply = Result<Value, String>;

/// How a window is dressed, for checks: see [`Browser::dressed`].
pub type Dressing = (usize, isize, bool, f64);

/// State the event loop and the worker share.
pub(super) struct Shared {
    pub token: String,
    /// Whether the window is open now: the person may close it, and it opens again when needed.
    pub open: AtomicBool,
    pub next_id: AtomicU64,
    pub pending: Mutex<HashMap<u64, Sender<Reply>>>,
    /// Each tab's loads, by tab id.
    pub loads: Mutex<HashMap<u32, Loads>>,
    pub loaded: Condvar,
    /// The tab acted on.
    pub active: AtomicU32,
    /// Numbers every load event across the tabs, so a load in another tab never reads as this
    /// tab's (a page that opened a tab has started a load: the new tab's).
    seq: AtomicU64,
    pub tabs: Mutex<Vec<Tab>>,
    next_panel: AtomicU32,
}

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
    fn answer(&self, id: u64, reply: Reply) {
        if let Some(sender) = self.pending.lock().unwrap().remove(&id) {
            let _ = sender.send(reply);
        }
    }

    /// The window was closed: every call waiting on it fails now, rather than at its timeout, and
    /// the load counts start over, so a window opened again waits for its own first document.
    fn closed(&self) {
        self.open.store(false, Ordering::SeqCst);
        for (_, sender) in self.pending.lock().unwrap().drain() {
            let _ = sender.send(Err(WINDOW_CLOSED.into()));
        }
        self.loads.lock().unwrap().clear();
        self.tabs.lock().unwrap().clear();
        self.loaded.notify_all();
    }

    pub(super) fn navigation_requested(&self, tab: u32) {
        let next = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        self.loads.lock().unwrap().entry(tab).or_default().requested = next;
        self.loaded.notify_all();
    }

    pub(super) fn page_load(&self, tab: u32, event: PageLoadEvent, url: &str) {
        let next = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut loads = self.loads.lock().unwrap();
            let loads = loads.entry(tab).or_default();
            match event {
                PageLoadEvent::Started => loads.started = next,
                PageLoadEvent::Finished => loads.finished = loads.started,
            }
        }
        if let Some(entry) = self.tabs.lock().unwrap().iter_mut().find(|t| t.id == tab) {
            entry.url = url.to_string();
        }
        self.loaded.notify_all();
    }

    pub(super) fn titled(&self, tab: u32, title: String) {
        if let Some(entry) = self.tabs.lock().unwrap().iter_mut().find(|t| t.id == tab) {
            entry.title = title;
        }
    }

    /// The tabs open now, `active` acted on; what is known of each is kept.
    pub(super) fn set_tabs(&self, active: u32, ids: &[u32]) {
        let mut tabs = self.tabs.lock().unwrap();
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

impl Browser {
    /// Open the window on `window.url` and wait for its document. One window per process: the
    /// browser shows one page.
    pub fn open(&self, window: Window) -> BrowserResult<WebviewDriver> {
        let (reply, opened) = mpsc::channel();
        self.proxy
            .send_event(Cmd::Open { window, reply })
            .map_err(|_| BrowserError::Unavailable("the browser host is gone".into()))?;
        opened
            .recv()
            .map_err(|_| BrowserError::Unavailable("the browser host is gone".into()))?
            .map_err(|error| {
                BrowserError::Unavailable(format!("could not open the browser window: {error}"))
            })?;
        Ok(WebviewDriver::new(self.proxy.clone(), self.shared.clone()))
    }

    /// A window titled `title` beside the browser's, showing `html` (a page of the caller's,
    /// which paints run frames in); it closes when the panel is dropped. It opens whether or not
    /// the browser's window is open, and closing either leaves the other.
    pub fn panel(&self, title: &str, html: String, ground: Option<[u8; 3]>) -> Panel {
        let id = self.shared.next_panel.fetch_add(1, Ordering::Relaxed);
        let proxy = self.proxy.clone();
        let panel = Panel::new(id, Box::new(move |cmd| proxy.send_event(cmd).is_ok()));
        let slot = Arc::clone(&panel.slot);
        let open = Cmd::Panel((id, title.to_string(), html, ground, slot));
        if self.proxy.send_event(open).is_err() {
            panel.slot.alive.store(false, Ordering::SeqCst);
        }
        panel
    }

    /// For checks: how the browser's window (no `panel`) or a panel's is dressed: its materials,
    /// the first one's state, whether the window is opaque, the shape its resizing keeps (width
    /// over height, 0 for none). `None` once it is closed.
    #[doc(hidden)]
    pub fn dressed(&self, panel: Option<&Panel>) -> Option<Dressing> {
        let (reply, answer) = mpsc::channel();
        let ask = Cmd::Dressed(panel.map(Panel::id), reply);
        self.proxy.send_event(ask).ok()?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()?
    }

    /// For checks: resize `panel`'s window, as the person would.
    #[doc(hidden)]
    pub fn resize_panel(&self, panel: &Panel, width: f64, height: f64) {
        let _ = (self.proxy).send_event(Cmd::PanelSize(panel.id(), width, height));
    }

    /// What `script` evaluates to in `panel`'s page, as JSON.
    pub fn peek(&self, panel: &Panel, script: &str) -> Result<Value, String> {
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.shared.pending.lock().unwrap().insert(id, sender);
        let script = script.to_string();
        let panel = Some(panel.id());
        let _ = (self.proxy).send_event(Cmd::Eval {
            id,
            script,
            failed: "",
            panel,
        });
        let reply = receiver.recv_timeout(std::time::Duration::from_secs(5));
        self.shared.pending.lock().unwrap().remove(&id);
        reply.unwrap_or_else(|_| Err("no answer".into()))
    }
}

/// Take the main thread for the browser's event loop and run `work` on a worker thread with a
/// handle that opens the window when it is needed. The event loop never returns, so the process
/// exits here, with the code `work` returns. When the person closes the window, every call waiting
/// on it fails, `on_close` runs (the app decides what that means for its task), and the window
/// opens again on its next use.
pub fn host<F>(on_close: fn(), work: F) -> !
where
    F: FnOnce(Browser) -> i32 + Send + 'static,
{
    // Changed on macOS only.
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut event_loop = EventLoopBuilder::<Cmd>::with_user_event().build();
    // The window never takes the keyboard: the person keeps typing where they were, and their keys
    // never land in the driven page.
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activate_ignoring_other_apps(false);
        // An accessory app: no Dock icon and no menu bar, whether its window is shown or not,
        // so a window that appears mid-task never pulls the person out of their terminal.
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }
    let proxy = event_loop.create_proxy();
    let shared = Arc::new(Shared {
        token: token(),
        open: AtomicBool::new(false),
        next_id: AtomicU64::new(1),
        pending: Mutex::new(HashMap::new()),
        loads: Mutex::new(HashMap::new()),
        loaded: Condvar::new(),
        active: AtomicU32::new(1),
        seq: AtomicU64::new(0),
        tabs: Mutex::new(vec![]),
        next_panel: AtomicU32::new(1),
    });
    let browser = Browser {
        proxy: proxy.clone(),
        shared: shared.clone(),
    };
    let exit = proxy.clone();
    std::thread::spawn(move || {
        let code = work(browser);
        let _ = exit.send_event(Cmd::Exit(code));
    });

    let mut view: Option<Tabs> = None;
    // The panels' windows, beside the browser's: closing one closes it alone.
    let mut panels: Vec<Open> = vec![];
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        let Event::UserEvent(command) = event else {
            let Event::WindowEvent {
                window_id, event, ..
            } = event
            else {
                return;
            };
            let browser = view.as_ref().map(|tabs| tabs.frame.native.id());
            let ids: Vec<WindowId> = panels.iter().map(|open| open.frame.native.id()).collect();
            match (route(window_id, browser, &ids), event) {
                (Route::Panel(index), WindowEvent::CloseRequested) => {
                    let closed = panels.remove(index);
                    closed.slot.alive.store(false, Ordering::SeqCst);
                }
                (Route::Browser, WindowEvent::CloseRequested) => {
                    view = None;
                    shared.closed();
                    on_close();
                }
                (Route::Panel(index), event) if frame::relayout(&event) => panels[index].layout(),
                (Route::Browser, event) if frame::relayout(&event) => {
                    if let Some(tabs) = &view {
                        tabs.layout();
                    }
                }
                // The system's appearance changed: a glass that follows it is tinted again.
                (Route::Panel(index), WindowEvent::ThemeChanged(_)) => panels[index].frame.retint(),
                (Route::Browser, WindowEvent::ThemeChanged(_)) => {
                    if let Some(tabs) = &view {
                        tabs.frame.retint();
                    }
                }
                _ => {}
            }
            return;
        };
        let command = match (command, &view) {
            (Cmd::Open { window, reply }, None) => {
                let opened = Tabs::open(&window, target, &shared, &proxy);
                shared.open.store(opened.is_ok(), Ordering::SeqCst);
                let _ = reply.send(opened.as_ref().map(|_| ()).map_err(String::clone));
                view = opened.ok();
                return;
            }
            (Cmd::Open { reply, .. }, Some(_)) => {
                let _ = reply.send(Err("the window is already open".into()));
                return;
            }
            (Cmd::Exit(code), _) => std::process::exit(code),
            (Cmd::Panel(opening), _) => {
                let alive = Arc::clone(&opening.4);
                match panel::open(opening, target, proxy.clone()) {
                    Ok(open) => panels.push(open),
                    Err(_) => alive.alive.store(false, Ordering::SeqCst),
                }
                return;
            }
            (Cmd::Paint(id), _) => {
                let open = panels.iter().find(|open| open.id == id);
                if let Some((open, frame)) = open.and_then(|o| Some((o, o.slot.latest()?))) {
                    let _ = open.webview.evaluate_script(&frame);
                }
                return;
            }
            (Cmd::Unpanel(id), _) => {
                panels.retain(|open| open.id != id);
                return;
            }
            (Cmd::Shape(id), _) => {
                if let Some(open) = panels.iter().find(|open| open.id == id) {
                    let proxy = proxy.clone();
                    let _ =
                        open.webview
                            .evaluate_script_with_callback(panel::SHAPE, move |answer| {
                                if let Some(size) = frame::shape(&answer) {
                                    let _ = proxy.send_event(Cmd::Reshape(id, size));
                                }
                            });
                }
                return;
            }
            (Cmd::Reshape(id, size), _) => {
                if let Some(open) = panels.iter().find(|open| open.id == id) {
                    open.reshape(size);
                }
                return;
            }
            (Cmd::Dressed(panel, reply), _) => {
                let frame = match panel {
                    Some(id) => panels.iter().find(|o| o.id == id).map(|o| &o.frame),
                    None => view.as_ref().map(|tabs| &tabs.frame),
                };
                let _ = reply.send(frame.map(|frame| {
                    frame.retint();
                    frame::dressed(&frame.native)
                }));
                return;
            }
            (Cmd::PanelSize(id, width, height), _) => {
                if let Some(open) = panels.iter().find(|open| open.id == id) {
                    open.frame
                        .native
                        .set_inner_size(LogicalSize::new(width, height));
                }
                return;
            }
            (
                Cmd::Eval {
                    id,
                    script,
                    panel: Some(panel),
                    ..
                },
                _,
            ) => {
                let answer = shared.clone();
                let reply = move |value: String| {
                    let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
                    answer.answer(id, Ok(value));
                };
                match panels.iter().find(|open| open.id == panel) {
                    Some(open) => drop(open.webview.evaluate_script_with_callback(&script, reply)),
                    None => shared.answer(id, Err(WINDOW_CLOSED.into())),
                }
                return;
            }
            (Cmd::Close, _) => {
                view = None;
                shared.closed();
                return;
            }
            (command, Some(_)) => command,
            // A driver exists only once the window opened: with none now, it was closed.
            (Cmd::Eval { id, .. }, None) => {
                shared.answer(id, Err(WINDOW_CLOSED.into()));
                return;
            }
            (_, None) => return,
        };
        let Some(tabs) = &mut view else {
            return;
        };
        match command {
            Cmd::Eval {
                id, script, failed, ..
            } => {
                let answer = shared.clone();
                let sent = tabs
                    .active()
                    .evaluate_script_with_callback(&script, move |started| {
                        if started.trim() != "true" {
                            answer.answer(id, Err(failed.into()));
                        }
                    });
                if let Err(error) = sent {
                    shared.answer(id, Err(format!("{failed}: {error}")));
                }
            }
            Cmd::Resize(width, height) => {
                tabs.frame
                    .native
                    .set_inner_size(LogicalSize::new(width, height));
                tabs.layout();
            }
            Cmd::Capture { clip, reply } => capture(tabs.active(), clip, reply),
            Cmd::Navigate(url) => {
                let _ = tabs.active().load_url(&url);
            }
            Cmd::NewTab { url, reply } => {
                let added = tabs.add(&url, &shared, &proxy);
                match reply {
                    Some(reply) => {
                        let _ = reply.send(added);
                    }
                    // A page's window past the tab limit opens where the page is, as before tabs.
                    None if added.is_err() => {
                        let _ = tabs.active().load_url(&url);
                    }
                    None => {}
                }
            }
            Cmd::SwitchTab { id, reply } => {
                let _ = reply.send(tabs.switch(id, &shared));
            }
            Cmd::CloseTab { id, reply } => {
                let closed = tabs.close(id, &shared);
                if closed == Ok(false) {
                    view = None;
                    shared.closed();
                }
                let _ = reply.send(closed);
            }
            Cmd::Open { .. } | Cmd::Exit(_) | Cmd::Close => {}
            Cmd::Panel(..) | Cmd::Paint(_) | Cmd::Unpanel(_) => {}
            Cmd::Dressed(..) | Cmd::PanelSize(..) | Cmd::Shape(_) | Cmd::Reshape(..) => {}
        }
    })
}

/// The answer to every call while the window is closed.
pub(super) const WINDOW_CLOSED: &str = "window_closed";
