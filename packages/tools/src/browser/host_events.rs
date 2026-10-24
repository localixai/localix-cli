//! The event loop: it takes the main thread, opens the window when asked, and carries out each
//! [`Cmd`]: the window's own, a panel's, and the browser's on its tabs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use std::sync::mpsc::SyncSender;

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::window::WindowId;

use super::super::deck::{After, Deck};
use super::super::frame;
use super::super::guard::token;
use super::super::panel::Opening;
use super::super::strip::{Click, Key, click};
use super::web::on_browser;
use super::{Browser, Cmd, Shared, WINDOW_CLOSED, Window};

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
    #[cfg_attr(
        not(target_os = "macos"),
        allow(unused_mut, reason = "only macOS changes the loop before it runs")
    )]
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
        groups: Mutex::new(HashMap::new()),
    });
    let browser = Browser {
        proxy: proxy.clone(),
        shared: Arc::clone(&shared),
    };
    let (exit, refused) = (proxy.clone(), proxy.clone());
    let worker = std::thread::Builder::new().name("llx-work".into());
    let started = worker.spawn(move || {
        let code = work(browser);
        let _ = exit.send_event(Cmd::Exit(code));
    });
    if started.is_err() {
        // No thread for the work: the process ends at once, as a failure (1, as anywhere).
        let _ = refused.send_event(Cmd::Exit(1));
    }

    let mut deck: Option<Deck> = None;
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            #[expect(
                clippy::exit,
                clippy::disallowed_methods,
                reason = "the event loop owns the main thread and never returns: the process ends \
                          here, with the code the run returned (main.rs hands it over)"
            )]
            Event::UserEvent(Cmd::Exit(code)) => std::process::exit(code),
            Event::UserEvent(command) => {
                let mut on = On {
                    deck: &mut deck,
                    target,
                    shared: &shared,
                    proxy: &proxy,
                    on_close,
                };
                on.command(command);
            }
            Event::WindowEvent {
                window_id, event, ..
            } => on_window(&mut deck, window_id, &event, &shared, on_close),
            _ => {}
        }
    })
}

/// The person did something to the window itself.
fn on_window(
    deck: &mut Option<Deck>,
    window_id: WindowId,
    event: &WindowEvent<'_>,
    shared: &Shared,
    on_close: fn(),
) {
    let Some(open) = deck.as_mut().filter(|d| d.frame.native.id() == window_id) else {
        return;
    };
    match event {
        // The person closed the window: the browser's calls fail and the task is told, and the
        // terminals' callers let their screens go.
        WindowEvent::CloseRequested => {
            open.let_go();
            let browser = open.web.is_some();
            *deck = None;
            shared.heard("window", CLOSED_WINDOW);
            if browser {
                shared.closed();
                on_close();
            }
        }
        event if frame::relayout(event) => open.layout(),
        // The system's appearance changed: a glass that follows it is tinted again.
        WindowEvent::ThemeChanged(_) => open.frame.retint(),
        _ => {}
    }
}

/// What a command is carried out with, on the event loop.
struct On<'a> {
    deck: &'a mut Option<Deck>,
    target: &'a EventLoopWindowTarget<Cmd>,
    shared: &'a Arc<Shared>,
    proxy: &'a EventLoopProxy<Cmd>,
    on_close: fn(),
}

impl On<'_> {
    fn command(&mut self, command: Cmd) {
        let (shared, proxy) = (self.shared, self.proxy);
        match command {
            Cmd::Open { window, reply } => self.open(&window, &reply),
            Cmd::Panel(opening) => self.panel(opening),
            Cmd::Paint(id) => {
                let open = self.deck.as_ref().and_then(|open| open.term(id));
                if let Some((open, frame)) = open.and_then(|o| Some((o, o.slot.latest()?))) {
                    let _ = open.webview.evaluate_script(&frame);
                }
            }
            Cmd::Unpanel(id) => {
                if let Some(open) = self.deck.as_mut() {
                    match open.remove_term(id) {
                        true => open.refresh(shared),
                        false => *self.deck = None,
                    }
                }
            }
            Cmd::Titled(id, title) => {
                if let Some(open) = self.deck.as_mut() {
                    open.titled(id, title);
                    open.refresh(shared);
                }
            }
            Cmd::Strip => {
                if let Some(open) = self.deck.as_ref() {
                    open.refresh(shared);
                }
            }
            Cmd::Clicked(said) => self.clicked(&said),
            Cmd::Sized(width, height, reply) => {
                let sized = self
                    .deck
                    .as_ref()
                    .ok_or("the window is not open")
                    .map(|open| {
                        let size = tao::dpi::LogicalSize::new(width, height);
                        open.frame.native.set_inner_size(size);
                        open.layout();
                    });
                let _ = reply.send(sized.map_err(String::from));
            }
            Cmd::Act(said, reply) => {
                let _ = reply.send(act(self.deck, said, shared, None));
            }
            Cmd::Listing(reply) => {
                let _ = reply.send(self.deck.as_ref().map(|open| open.listing(shared)));
            }
            Cmd::Dressed(reply) => {
                let _ = reply.send(self.deck.as_ref().map(|open| {
                    open.frame.retint();
                    frame::dressed(&open.frame.native)
                }));
            }
            Cmd::Eval {
                id,
                script,
                panel: Some(panel),
                ..
            } => self.eval_panel(id, &script, panel),
            Cmd::Close => {
                // The agent closed the browser: the terminals stay.
                if let Some(open) = self.deck.as_mut() {
                    shared.closed();
                    match open.remove_web() {
                        true => open.refresh(shared),
                        false => *self.deck = None,
                    }
                }
            }
            command => on_browser(command, self.deck, shared, proxy),
        }
    }

    fn open(&mut self, window: &Window, reply: &SyncSender<Result<(), String>>) {
        let (shared, proxy) = (self.shared, self.proxy);
        let opened = match self.deck.as_mut() {
            Some(open) if open.web.is_some() => Err("the window is already open".into()),
            Some(open) => open.open_web(window, shared, proxy),
            None => Deck::for_web(self.target, window, shared, proxy).map(|open| {
                *self.deck = Some(open);
            }),
        };
        shared.open.store(opened.is_ok(), Ordering::SeqCst);
        let _ = reply.send(opened.clone());
        if let (Ok(()), Some(open)) = (opened, self.deck.as_ref()) {
            open.refresh(shared);
        }
    }

    fn panel(&mut self, opening: Opening) {
        let alive = Arc::clone(&opening.4);
        let opened = match self.deck.as_mut() {
            Some(open) => open.add_term(opening, self.proxy),
            None => Deck::for_term(self.target, opening, self.proxy).map(|open| {
                *self.deck = Some(open);
            }),
        };
        match (opened, self.deck.as_ref()) {
            (Ok(()), Some(open)) => open.refresh(self.shared),
            _ => alive.alive.store(false, Ordering::SeqCst),
        }
    }

    /// The person's click: closing the browser's last tab is closing its window.
    fn clicked(&mut self, said: &str) {
        if let Some(said) = click(said)
            && act(self.deck, said, self.shared, Some(self.on_close)).is_ok()
            && let Click::Close(key) = said
        {
            let what = match key {
                Key::Web(_) => CLOSED_PAGE,
                Key::Term(_) => CLOSED_TERMINAL,
            };
            self.shared.heard(&key.name(), what);
        }
    }

    /// `script` in a panel's page, or in the strip's (`STRIP`, for checks).
    fn eval_panel(&self, id: u64, script: &str, panel: u32) {
        let answer = Arc::clone(self.shared);
        let reply = move |value: String| {
            let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
            answer.answer(id, Ok(value));
        };
        match self.deck.as_ref().and_then(|open| open.page(panel)) {
            Some(page) => drop(page.evaluate_script_with_callback(script, reply)),
            None => self.shared.answer(id, Err(WINDOW_CLOSED.into())),
        }
    }
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
