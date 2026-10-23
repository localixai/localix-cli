//! The event loop: it takes the main thread, opens the window when asked, and carries out each
//! [`Cmd`]: the window's own, a panel's, and the browser's on its tabs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use serde_json::Value;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};

use super::super::deck::{After, Deck};
use super::super::frame;
use super::super::guard::token;
use super::super::strip::{Click, Key, click};
use super::web::on_browser;
use super::{Browser, Cmd, Shared, WINDOW_CLOSED};

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
