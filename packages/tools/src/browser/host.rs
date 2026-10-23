//! The native window: a `tao` event loop on the main thread that opens one window when asked (it
//! exists only once it is needed): the browser's tabs and the terminal sessions' screens are its
//! tabs (deck.rs), with a `wry` WebView each. The caller's work runs on a worker thread and
//! reaches the page through [`Cmd`]s on the event loop's proxy; answers come back over the page's
//! IPC channel, matched by call id and a per-run token the page cannot read.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use wry::PageLoadEvent;

use super::capture::{Clip, Shot, capture};
use super::deck::{After, Deck, Shown};
use super::frame;
use super::guard::token;
use super::panel::{self, Panel};
use super::strip::{Click, Key, click};
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
    Dressed(Sender<Option<Dressing>>),
    /// The window's tabs; `None` while it is closed.
    Listing(Sender<Option<Vec<Listed>>>),
    /// Size the whole window, in points.
    Sized(f64, f64, Sender<Result<(), String>>),
    /// Show or close a tab by its key, for the agent: the task goes on whatever closes.
    Act(Click, Sender<Result<(), String>>),
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
}

/// How a window is dressed, for checks: see [`Browser::dressed`].
pub type Dressing = (usize, isize, bool, f64, bool);

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
    /// Who hears what the person does in the window.
    listener: Mutex<Option<Listener>>,
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
        if let Some(listen) = self.listener.lock().unwrap().as_ref() {
            listen(key, what);
        }
    }

    fn answer(&self, id: u64, reply: Reply) {
        if let Some(sender) = self.pending.lock().unwrap().remove(&id) {
            let _ = sender.send(reply);
        }
    }

    /// The window was closed: every call waiting on it fails now, rather than at its timeout, and
    /// the load counts start over, so a window opened again waits for its own first document.
    pub(super) fn closed(&self) {
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
        let (reply, answer) = mpsc::channel();
        self.proxy.send_event(Cmd::Dressed(reply)).ok()?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()?
    }

    /// Hear what the person does in the window: a tab or the window closed, by key (`window` for
    /// the whole of it) and in words. It runs on the window's thread: it must not wait.
    pub fn listen(&self, listener: impl Fn(&str, &str) + Send + Sync + 'static) {
        *self.shared.listener.lock().unwrap() = Some(Box::new(listener));
    }

    /// The window's tabs, in the strip's order; `None` while it is closed.
    pub fn listing(&self) -> Option<Vec<Listed>> {
        let (reply, answer) = mpsc::channel();
        self.proxy.send_event(Cmd::Listing(reply)).ok()?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()?
    }

    /// Size the whole window to `width` by `height` points, as the person would.
    pub fn resize_window(&self, width: f64, height: f64) -> Result<(), String> {
        let (reply, answer) = mpsc::channel();
        let gone = || "the window is not open".to_string();
        let ask = Cmd::Sized(width, height, reply);
        self.proxy.send_event(ask).map_err(|_| gone())?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| gone())?
    }

    /// Show the tab `id` (`w1`, `t2`), or take it out of the window (`close`).
    pub fn act(&self, id: &str, close: bool) -> Result<(), String> {
        let key = Key::parse(id).ok_or_else(|| format!("there is no tab {id}"))?;
        let click = if close {
            Click::Close(key)
        } else {
            Click::Show(key)
        };
        let (reply, answer) = mpsc::channel();
        let gone = || "the window is not open".to_string();
        self.proxy
            .send_event(Cmd::Act(click, reply))
            .map_err(|_| gone())?;
        answer
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| gone())?
    }

    /// For checks: what the person's click on the strip says (`show t1`, `close w2`).
    #[doc(hidden)]
    pub fn click(&self, said: &str) {
        let _ = self.proxy.send_event(Cmd::Clicked(said.to_string()));
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
        listener: Mutex::new(None),
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

    let mut deck: Option<Deck> = None;
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        let Event::UserEvent(command) = event else {
            let Event::WindowEvent {
                window_id, event, ..
            } = event
            else {
                return;
            };
            let Some(open) = deck.as_mut().filter(|d| d.frame.native.id() == window_id) else {
                return;
            };
            match event {
                // The person closed the window: the browser's calls fail and the task is told,
                // and the terminals' callers let their screens go.
                WindowEvent::CloseRequested => {
                    open.let_go();
                    let browser = open.web.is_some();
                    deck = None;
                    shared.heard("window", CLOSED_WINDOW);
                    if browser {
                        shared.closed();
                        on_close();
                    }
                }
                event if frame::relayout(&event) => open.layout(),
                // The system's appearance changed: a glass that follows it is tinted again.
                WindowEvent::ThemeChanged(_) => open.frame.retint(),
                _ => {}
            }
            return;
        };
        match command {
            Cmd::Open { window, reply } => {
                let opened = match deck.as_mut() {
                    Some(open) if open.web.is_some() => Err("the window is already open".into()),
                    Some(open) => open.open_web(&window, &shared, &proxy),
                    None => Deck::for_web(target, &window, &shared, &proxy).map(|open| {
                        deck = Some(open);
                    }),
                };
                shared.open.store(opened.is_ok(), Ordering::SeqCst);
                let _ = reply.send(opened.clone());
                if let (Ok(()), Some(open)) = (opened, &deck) {
                    open.refresh(&shared);
                }
            }
            Cmd::Exit(code) => std::process::exit(code),
            Cmd::Panel(opening) => {
                let alive = Arc::clone(&opening.4);
                let opened = match deck.as_mut() {
                    Some(open) => open.add_term(opening, &proxy),
                    None => Deck::for_term(target, opening, &proxy).map(|open| {
                        deck = Some(open);
                    }),
                };
                match (opened, &deck) {
                    (Ok(()), Some(open)) => open.refresh(&shared),
                    _ => alive.alive.store(false, Ordering::SeqCst),
                }
            }
            Cmd::Paint(id) => {
                let open = deck.as_ref().and_then(|open| open.term(id));
                if let Some((open, frame)) = open.and_then(|o| Some((o, o.slot.latest()?))) {
                    let _ = open.webview.evaluate_script(&frame);
                }
            }
            Cmd::Unpanel(id) => {
                if let Some(open) = deck.as_mut() {
                    match open.remove_term(id) {
                        true => open.refresh(&shared),
                        false => deck = None,
                    }
                }
            }
            Cmd::Titled(id, title) => {
                if let Some(open) = deck.as_mut() {
                    open.titled(id, title);
                    open.refresh(&shared);
                }
            }
            Cmd::Strip => {
                if let Some(open) = &deck {
                    open.refresh(&shared);
                }
            }
            // The person's click: closing the browser's last tab is closing its window.
            Cmd::Clicked(said) => {
                if let Some(said) = click(&said)
                    && act(&mut deck, said, &shared, Some(on_close)).is_ok()
                    && let Click::Close(key) = said
                {
                    let what = match key {
                        Key::Web(_) => CLOSED_PAGE,
                        Key::Term(_) => CLOSED_TERMINAL,
                    };
                    shared.heard(&key.name(), what);
                }
            }
            Cmd::Sized(width, height, reply) => {
                let sized = deck.as_ref().ok_or("the window is not open").map(|open| {
                    let size = tao::dpi::LogicalSize::new(width, height);
                    open.frame.native.set_inner_size(size);
                    open.layout();
                });
                let _ = reply.send(sized.map_err(String::from));
            }
            Cmd::Act(said, reply) => {
                let _ = reply.send(act(&mut deck, said, &shared, None));
            }
            Cmd::Listing(reply) => {
                let _ = reply.send(deck.as_ref().map(|open| open.listing(&shared)));
            }
            Cmd::Dressed(reply) => {
                let _ = reply.send(deck.as_ref().map(|open| {
                    open.frame.retint();
                    frame::dressed(&open.frame.native)
                }));
            }
            Cmd::Eval {
                id,
                script,
                panel: Some(panel),
                ..
            } => {
                let answer = shared.clone();
                let reply = move |value: String| {
                    let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
                    answer.answer(id, Ok(value));
                };
                match deck.as_ref().and_then(|open| open.term(panel)) {
                    Some(open) => drop(open.webview.evaluate_script_with_callback(&script, reply)),
                    None => shared.answer(id, Err(WINDOW_CLOSED.into())),
                }
            }
            Cmd::Close => {
                // The agent closed the browser: the terminals stay.
                if let Some(open) = deck.as_mut() {
                    shared.closed();
                    match open.remove_web() {
                        true => open.refresh(&shared),
                        false => deck = None,
                    }
                }
            }
            command => on_browser(command, &mut deck, &shared, &proxy),
        }
    })
}

/// A command of the browser's own, on its tabs in the window.
fn on_browser(
    command: Cmd,
    deck: &mut Option<Deck>,
    shared: &Arc<Shared>,
    proxy: &EventLoopProxy<Cmd>,
) {
    // A driver exists only once the browser opened: with none now, it was closed.
    let Some(open) = deck.as_mut().filter(|open| open.web.is_some()) else {
        if let Cmd::Eval { id, .. } = command {
            shared.answer(id, Err(WINDOW_CLOSED.into()));
        }
        return;
    };
    let tabs = open.web.as_mut().expect("the browser is open");
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
            return;
        }
        Cmd::Resize(width, height) => open.resize(width, height),
        Cmd::Capture { clip, reply } => return capture(tabs.active(), clip, reply),
        // What the agent does in the browser is shown: the window goes back to it.
        Cmd::Navigate(url) => {
            let _ = tabs.active().load_url(&url);
            open.show(Shown::Web);
        }
        Cmd::NewTab { url, reply } => {
            let added = tabs.add(&open.frame.native, &url, shared, proxy);
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
            open.show(Shown::Web);
        }
        Cmd::SwitchTab { id, reply } => {
            let _ = reply.send(tabs.switch(id, shared));
            open.show(Shown::Web);
        }
        Cmd::CloseTab { id, reply } => {
            let closed = tabs.close(id, shared);
            let emptied = closed == Ok(false);
            let _ = reply.send(closed);
            if emptied {
                shared.closed();
                if !open.remove_web() {
                    *deck = None;
                    return;
                }
            }
        }
        _ => return,
    }
    open.refresh(shared);
}

/// Show or close the tab `said` names. A person's click closing the browser's last tab stops the
/// task as closing its window does (`on_close`); the agent's goes on.
fn act(
    deck: &mut Option<Deck>,
    said: Click,
    shared: &Shared,
    on_close: Option<fn()>,
) -> Result<(), String> {
    let open = deck.as_mut().ok_or("the window is not open")?;
    let after = open.act(said, shared)?;
    if let (After::BrowserGone { .. }, Some(on_close)) = (&after, on_close) {
        on_close();
    }
    match after {
        After::Empty | After::BrowserGone { empty: true } => *deck = None,
        _ => open.refresh(shared),
    }
    Ok(())
}

/// What the person's closing says, to whoever listens.
const CLOSED_WINDOW: &str = "The person closed the window: its tabs are gone (terminal programs \
run on, out of sight).";
const CLOSED_PAGE: &str = "The person closed this browser tab.";
const CLOSED_TERMINAL: &str = "The person closed this tab: its program runs on, out of sight.";

/// The answer to every call while the window is closed.
pub(super) const WINDOW_CLOSED: &str = "window_closed";
