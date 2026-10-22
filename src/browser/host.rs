//! The native window: a `tao` event loop on the main thread hosting one `wry` WebView. The agent
//! runs on a worker thread and reaches the page through [`Cmd`]s on the loop's proxy; answers come
//! back over the page's IPC channel, matched by call id and a per-run token the page cannot read.

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::{NewWindowResponse, PageLoadEvent, WebViewBuilder};

use super::guard::{allowed, token};
use super::platform::{conceal, keep_rendering, user_agent};
use super::viewport::Viewport;
use super::webview::WebviewDriver;

/// One script from four files: snapshot, actions, keys, then evaluate and the IPC entry point.
const PROBE: &str = concat!(
    include_str!("probe/snapshot.js"),
    include_str!("probe/act.js"),
    include_str!("probe/keys.js"),
    include_str!("probe/evaluate.js"),
);
/// Off-screen placement of a headless window where the OS allows it (see platform.rs).
#[cfg(not(target_os = "macos"))]
pub(super) const WIDTH: f64 = 1280.0;
#[cfg(not(target_os = "macos"))]
pub(super) const HEIGHT: f64 = 860.0;

/// How the window is opened.
pub struct Window {
    pub url: String,
    pub title: String,
    /// Keep the window off screen and out of the Dock; the page still renders.
    pub headless: bool,
    /// The page's size, and the user agent of a tablet or phone.
    pub viewport: Viewport,
}

pub(super) enum Cmd {
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
    Exit(i32),
}

pub(super) type Reply = Result<Value, String>;

/// State the loop and the worker share.
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

/// Open the window and run `work` on a worker thread with a driver for it. The process exits with
/// the code `work` returns, once the window is closed.
pub fn run<F>(window: Window, work: F) -> !
where
    F: FnOnce(WebviewDriver) -> i32 + Send + 'static,
{
    let mut event_loop = EventLoopBuilder::<Cmd>::with_user_event().build();
    // The window never takes the keyboard: the person keeps typing where they were, and their keys
    // never land in the agent's page. A headless run also stays out of the Dock.
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activate_ignoring_other_apps(false);
        if window.headless {
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
    let native = builder
        .build(&event_loop)
        .unwrap_or_else(|error| fail(&error.to_string()));
    let webview = webview(&window, &shared, proxy.clone())
        .and_then(|builder| build(builder, &native))
        .unwrap_or_else(|error| fail(&error.to_string()));
    // Shows the window too, without making it the key window (tao's set_visible would).
    keep_rendering(&webview, window.headless);
    #[cfg(not(target_os = "macos"))]
    if window.headless {
        native.set_visible(true);
    }

    let driver = WebviewDriver::new(proxy.clone(), shared.clone());
    std::thread::spawn(move || {
        let code = work(driver);
        let _ = proxy.send_event(Cmd::Exit(code));
    });

    let shared_loop = shared.clone();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(Cmd::Call { id, name, args }) => {
                let script = format!(
                    "window.__lxsurf_call ? window.__lxsurf_call({id}, {name:?}, {args}) : false"
                );
                let shared = shared_loop.clone();
                let sent = webview.evaluate_script_with_callback(&script, move |accepted| {
                    if accepted.trim() != "true" {
                        shared.answer(id, Err("probe_not_ready".into()));
                    }
                });
                if let Err(error) = sent {
                    shared_loop.answer(id, Err(format!("probe_not_ready: {error}")));
                }
            }
            Event::UserEvent(Cmd::Eval { id, script }) => {
                let shared = shared_loop.clone();
                let sent = webview.evaluate_script_with_callback(&script, move |started| {
                    if started.trim() != "true" {
                        shared.answer(id, Err("script_not_started".into()));
                    }
                });
                if let Err(error) = sent {
                    shared_loop.answer(id, Err(format!("script_not_started: {error}")));
                }
            }
            Event::UserEvent(Cmd::Resize(width, height)) => {
                native.set_inner_size(LogicalSize::new(width, height));
            }
            Event::UserEvent(Cmd::Navigate(url)) => {
                let _ = webview.load_url(&url);
            }
            Event::UserEvent(Cmd::Exit(code)) => std::process::exit(code),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => std::process::exit(crate::exit::INTERRUPTED),
            _ => {}
        }
    })
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
            if std::env::var_os("LXSURF_DEBUG").is_some() {
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
            if std::env::var_os("LXSURF_DEBUG").is_some() {
                eprintln!("navigation requested: {url} (allowed: {allow})");
            }
            if allow {
                requests.navigation_requested();
            }
            allow
        })
        .with_download_started_handler(|_url, _path| false)
        // A new tab or popup opens in this window instead: the agent drives one page.
        .with_new_window_req_handler(move |url, _features| {
            if allowed(&url) {
                let _ = proxy.send_event(Cmd::Navigate(url));
            }
            NewWindowResponse::Deny
        })
        .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled);
    if let Some(agent) = window
        .viewport
        .user_agent
        .map(String::from)
        .or_else(user_agent)
    {
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

fn fail(message: &str) -> ! {
    eprintln!("lxsurf: could not open the browser window: {message}");
    std::process::exit(crate::exit::ENVIRONMENT)
}
