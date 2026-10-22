//! The native window: a `tao` event loop on the main thread that opens one `wry` WebView when asked
//! (the window exists only once it is needed). The caller's work runs on a worker thread and
//! reaches the page through [`Cmd`]s on the event loop's proxy; answers come back over the page's
//! IPC channel, matched by call id and a per-run token the page cannot read.

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::window::WindowBuilder;
use wry::{NewWindowResponse, PageLoadEvent, WebViewBuilder};

use super::guard::{allowed, token};
use super::platform::{capture, conceal, keep_rendering, user_agent};
use super::viewport::Viewport;
use super::webview::WebviewDriver;
use super::{BrowserError, BrowserResult};

/// One script from five files: snapshot, same-origin frames, the outline, actions, then evaluate
/// and the IPC entry point.
const PROBE: &str = concat!(
    include_str!("probe/snapshot.js"),
    include_str!("probe/frames.js"),
    include_str!("probe/outline.js"),
    include_str!("probe/act.js"),
    include_str!("probe/evaluate.js"),
);
/// Off-screen placement of a headless window where the OS allows it (see platform.rs).
#[cfg(not(target_os = "macos"))]
pub(super) const WIDTH: f64 = 1280.0;
#[cfg(not(target_os = "macos"))]
pub(super) const HEIGHT: f64 = 860.0;

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
    Call {
        id: u64,
        name: &'static str,
        args: String,
    },
    /// A script built by the driver that hands its promise to the probe; `false` or a parse error
    /// answers the call at once with `script_not_started`.
    Eval {
        id: u64,
        script: String,
    },
    Navigate(String),
    /// Reframe the page (`viewport`): width, height.
    Resize(f64, f64),
    /// A PNG of what the page shows; the reply comes when the WebView has drawn it.
    Capture(Sender<Result<Vec<u8>, String>>),
    Exit(i32),
}

pub(super) type Reply = Result<Value, String>;

/// State the event loop and the worker share.
pub(super) struct Shared {
    pub token: String,
    pub next_id: AtomicU64,
    pub pending: Mutex<HashMap<u64, Sender<Reply>>>,
    pub loads: Mutex<Loads>,
    pub loaded: Condvar,
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

    fn navigation_requested(&self) {
        self.loads.lock().unwrap().requested += 1;
        self.loaded.notify_all();
    }

    fn page_load(&self, event: PageLoadEvent) {
        let mut loads = self.loads.lock().unwrap();
        match event {
            PageLoadEvent::Started => loads.started += 1,
            PageLoadEvent::Finished => loads.finished = loads.started,
        }
        self.loaded.notify_all();
    }

    /// A message from the page: `{t, id, ok}` or `{t, id, err}`. Anything else is ignored.
    fn ipc(&self, body: &str) {
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
}

/// Take the main thread for the browser's event loop and run `work` on a worker thread with a
/// handle that opens the window when it is needed. The process exits with the code `work`
/// returns, or with `closed` when the person closes the window. `headless` is decided here: an
/// invisible window keeps the app out of the Dock.
pub fn host<F>(headless: bool, closed: i32, work: F) -> !
where
    F: FnOnce(Browser) -> i32 + Send + 'static,
{
    let mut event_loop = EventLoopBuilder::<Cmd>::with_user_event().build();
    // The window never takes the keyboard: the person keeps typing where they were, and their keys
    // never land in the driven page.
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activate_ignoring_other_apps(false);
        if headless {
            event_loop.set_activation_policy(ActivationPolicy::Accessory);
        }
    }
    let proxy = event_loop.create_proxy();
    let shared = Arc::new(Shared {
        token: token(),
        next_id: AtomicU64::new(1),
        pending: Mutex::new(HashMap::new()),
        loads: Mutex::new(Loads::default()),
        loaded: Condvar::new(),
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

    let mut view: Option<(tao::window::Window, wry::WebView)> = None;
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        let Event::UserEvent(command) = event else {
            if let Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } = event
            {
                std::process::exit(closed);
            }
            return;
        };
        let command = match (command, &view) {
            (Cmd::Open { window, reply }, None) => {
                let opened = open(&window, target, &shared, proxy.clone());
                let _ = reply.send(opened.as_ref().map(|_| ()).map_err(String::clone));
                view = opened.ok();
                return;
            }
            (Cmd::Open { reply, .. }, Some(_)) => {
                let _ = reply.send(Err("the window is already open".into()));
                return;
            }
            (Cmd::Exit(code), _) => std::process::exit(code),
            (command, Some(_)) => command,
            // Nothing to call before the window opens.
            (Cmd::Call { id, .. } | Cmd::Eval { id, .. }, None) => {
                shared.answer(id, Err("probe_not_ready: no window yet".into()));
                return;
            }
            (_, None) => return,
        };
        let Some((native, webview)) = &view else {
            return;
        };
        match command {
            Cmd::Call { id, name, args } => {
                let script = format!(
                    "window.__dloop_call ? window.__dloop_call({id}, {name:?}, {args}) : false"
                );
                let answer = shared.clone();
                let sent = webview.evaluate_script_with_callback(&script, move |accepted| {
                    if accepted.trim() != "true" {
                        answer.answer(id, Err("probe_not_ready".into()));
                    }
                });
                if let Err(error) = sent {
                    shared.answer(id, Err(format!("probe_not_ready: {error}")));
                }
            }
            Cmd::Eval { id, script } => {
                let answer = shared.clone();
                let sent = webview.evaluate_script_with_callback(&script, move |started| {
                    if started.trim() != "true" {
                        answer.answer(id, Err("script_not_started".into()));
                    }
                });
                if let Err(error) = sent {
                    shared.answer(id, Err(format!("script_not_started: {error}")));
                }
            }
            Cmd::Resize(width, height) => native.set_inner_size(LogicalSize::new(width, height)),
            Cmd::Capture(reply) => capture(webview, reply),
            Cmd::Navigate(url) => {
                let _ = webview.load_url(&url);
            }
            Cmd::Open { .. } | Cmd::Exit(_) => {}
        }
    })
}

/// Open `window` and run `work` with its driver: the browser alone, for tests and examples. A
/// window that cannot open ends the process with `ENVIRONMENT_EXIT`.
pub fn run<F>(window: Window, work: F) -> !
where
    F: FnOnce(WebviewDriver) -> i32 + Send + 'static,
{
    host(window.headless, CLOSED_EXIT, move |browser| {
        match browser.open(window) {
            Ok(driver) => work(driver),
            Err(error) => {
                eprintln!("{error}");
                ENVIRONMENT_EXIT
            }
        }
    })
}

const CLOSED_EXIT: i32 = 130;
const ENVIRONMENT_EXIT: i32 = 3;

/// The window and its WebView, shown without taking focus.
fn open(
    window: &Window,
    target: &EventLoopWindowTarget<Cmd>,
    shared: &Arc<Shared>,
    proxy: EventLoopProxy<Cmd>,
) -> Result<(tao::window::Window, wry::WebView), String> {
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
    let webview = webview(window, shared, proxy)
        .and_then(|builder| build(builder, &native))
        .map_err(|error| error.to_string())?;
    // Shows the window too, without making it the key window (tao's set_visible would).
    keep_rendering(&webview, window.headless);
    #[cfg(not(target_os = "macos"))]
    if window.headless {
        native.set_visible(true);
    }
    Ok((native, webview))
}

fn webview<'a>(
    window: &'a Window,
    shared: &Arc<Shared>,
    proxy: EventLoopProxy<Cmd>,
) -> wry::Result<WebViewBuilder<'a>> {
    let ipc = shared.clone();
    let loads = shared.clone();
    let requests = shared.clone();
    let mut builder = WebViewBuilder::new()
        .with_url(&window.url)
        .with_incognito(true)
        .with_initialization_script_for_main_only(
            PROBE.replace("__TOKEN__", &shared.token).replace(
                "__HEADLESS__",
                if window.headless { "true" } else { "false" },
            ),
            true,
        )
        .with_ipc_handler(move |request| ipc.ipc(request.body()))
        .with_on_page_load_handler(move |event, url| {
            if std::env::var_os("BROWSER_DEBUG").is_some() {
                let kind = if matches!(event, PageLoadEvent::Started) {
                    "started"
                } else {
                    "finished"
                };
                eprintln!("load {kind}: {url}");
            }
            loads.page_load(event)
        })
        .with_navigation_handler(move |url| {
            let allow = allowed(&url);
            if std::env::var_os("BROWSER_DEBUG").is_some() {
                eprintln!("navigation requested: {url} (allowed: {allow})");
            }
            if allow {
                requests.navigation_requested();
            }
            allow
        })
        .with_download_started_handler(|_url, _path| false)
        // A new tab or popup opens in this window instead: one page at a time.
        .with_new_window_req_handler(move |url, _features| {
            if allowed(&url) {
                let _ = proxy.send_event(Cmd::Navigate(url));
            }
            NewWindowResponse::Deny
        })
        .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled);
    if let Some(agent) = user_agent() {
        builder = builder.with_user_agent(agent);
    }
    Ok(builder)
}

#[cfg(not(target_os = "linux"))]
fn build(builder: WebViewBuilder, window: &tao::window::Window) -> wry::Result<wry::WebView> {
    builder.build(window)
}

#[cfg(target_os = "linux")]
fn build(builder: WebViewBuilder, window: &tao::window::Window) -> wry::Result<wry::WebView> {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;
    builder.build_gtk(window.default_vbox().expect("tao window has a vbox"))
}
