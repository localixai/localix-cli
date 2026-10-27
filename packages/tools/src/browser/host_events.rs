//! The event loop: it takes the main thread, opens the window when asked, and carries out each
//! [`Cmd`]: the window's own, a panel's, and the browser's on its tabs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use std::sync::mpsc::SyncSender;

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};

use super::super::comments::Comments;
use super::super::deck::Deck;
use super::super::frame;
use super::super::guard::token;
use super::super::history::History;
use super::super::panel::Opening;
use super::super::placement::Kept;
use super::super::platform;
use super::super::presence::Event as Presence;
use super::super::strip::{Click, Key};
use super::presence::{Presenter, Stage};
use super::web::on_browser;
use super::window::on_window;
use super::{Browser, Cmd, Shared, Window};

/// Take the main thread for the browser's event loop and run `work` on a worker thread with a
/// handle that opens the window when it is needed. The event loop never returns, so the process
/// exits here, with the code `work` returns (the `i32` is the type the headless build's `host`
/// shares, which does return). When the person closes the window, every call waiting on it fails,
/// `on_close` runs (the app decides what that means for its task), and the window opens again on
/// its next use.
pub fn host<F>(on_close: fn(), work: F) -> i32
where
    F: FnOnce(Browser) -> i32 + Send + 'static,
{
    // Changed on macOS only.
    #[cfg_attr(
        not(target_os = "macos"),
        expect(unused_mut, reason = "only macOS changes the loop before it runs")
    )]
    let mut event_loop = EventLoopBuilder::<Cmd>::with_user_event().build();
    // The window never takes the keyboard: the person keeps typing where they were, and their keys
    // never land in the driven page.
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activate_ignoring_other_apps(false);
        // A background app until a window is shown (`platform::dock`): no Dock icon, no menu bar,
        // and never the active app, so nothing mid-task takes the keys of the person's terminal.
        event_loop.set_activation_policy(ActivationPolicy::Prohibited);
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
        history: History::default(),
        comments: Comments::default(),
        kept: Mutex::new(Kept::default()),
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
    let mut presenter = Presenter::new();
    let mut docked = false;
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "tao's events: foreign, non-exhaustive"
        )]
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
                    presenter: &mut presenter,
                    target,
                    shared: &shared,
                    proxy: &proxy,
                    on_close,
                };
                on.command(command);
            }
            Event::WindowEvent {
                window_id, event, ..
            } => on_window(
                (&mut deck, &presenter),
                window_id,
                &event,
                &shared,
                on_close,
            ),
            _ => {}
        }
        let mut stage = Stage {
            deck: &mut deck,
            target,
            shared: &shared,
            proxy: &proxy,
        };
        presenter.sync(&mut stage);
        let shown = deck.as_ref().is_some_and(Deck::is_shown);
        if shown != docked {
            docked = shown;
            platform::dock(target, shown);
        }
    })
}

#[path = "host_clicks.rs"]
mod clicks;
use clicks::act;

/// What a command is carried out with, on the event loop.
struct On<'a> {
    deck: &'a mut Option<Deck>,
    presenter: &'a mut Presenter,
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
                    match open.end_term(id) {
                        true => open.refresh(shared),
                        false => *self.deck = None,
                    }
                }
            }
            Cmd::Titled(id, title) => {
                shared.history.titled(Key::Term(id), &title);
                if let Some(open) = self.deck.as_mut() {
                    open.titled(id, title);
                    open.refresh(shared);
                }
            }
            Cmd::Strip => {
                if let Some(open) = self.deck.as_ref() {
                    open.refresh(shared);
                }
                self.renamed();
            }
            Cmd::Tinted(under, ground) => {
                if let Some(open) = self.deck.as_ref() {
                    open.tinted(under, ground);
                }
            }
            Cmd::Presence(event) => self.presence(event),
            Cmd::Pictured(shot) => self.pictured(&shot),
            Cmd::Presented(reply) => {
                let _ = reply.send(self.presenter.name());
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
                let done = act(self.deck, said, shared, None);
                // The agent showed a tab: the mini blinks, and never opens.
                if done.is_ok() && matches!(said, Click::Show(_)) {
                    self.presence(Presence::Show);
                }
                let _ = reply.send(done);
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
            command @ (Cmd::Eval { .. }
            | Cmd::Navigate(..)
            | Cmd::NewTab { .. }
            | Cmd::SwitchTab { .. }
            | Cmd::CloseTab { .. }
            | Cmd::Resize(..)
            | Cmd::Capture { .. }
            | Cmd::Noted(_)
            | Cmd::Exit(_)) => on_browser(command, self.deck, shared, proxy),
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
        // Whether the window is open now: a second open refused leaves the first one open.
        let web = self.deck.as_ref().is_some_and(|open| open.web.is_some());
        shared.open.store(web, Ordering::SeqCst);
        let _ = reply.send(opened.clone());
        if let (Ok(()), Some(open)) = (opened, self.deck.as_ref()) {
            open.refresh(shared);
        }
    }

    fn panel(&mut self, opening: Opening) {
        let alive = Arc::clone(&opening.4);
        self.shared.history.opened(Key::Term(opening.0), &opening.1);
        let opened = match self.deck.as_mut() {
            Some(open) => open.add_term(opening, self.proxy),
            None => Deck::for_term(self.target, opening, self.shared, self.proxy).map(|open| {
                *self.deck = Some(open);
            }),
        };
        match (opened, self.deck.as_ref()) {
            (Ok(()), Some(open)) => open.refresh(self.shared),
            _ => alive.alive.store(false, Ordering::SeqCst),
        }
    }
}
