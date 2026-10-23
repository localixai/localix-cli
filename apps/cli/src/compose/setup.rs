//! What a session is built from ([`Setup`]), what its lanes share ([`Shared`]), and what one
//! lane is built from beside it ([`Seat`]).

use std::sync::Arc;

use contracts::history::Session;
use serde_json::Value;
use session::Sessions;
use tools::browser::{Browser, Page};
use tools::workspace::Workspace;

use crate::choice::Choice;
use crate::cli::RunOptions;
use crate::mode::Mode;
use crate::tui::Ui;

/// Everything a session is built from, decided before the browser's event loop takes the main
/// thread (so a bad argument still fails fast, with no window machinery started).
pub struct Setup {
    pub options: RunOptions,
    /// The provider, model and reasoning level the agent thinks with.
    pub choice: Choice,
    pub workspace: Workspace,
    /// The agent's context block: the environment, the instructions, an earlier session.
    pub context: String,
    /// The same with no earlier session: what `/new` starts over with.
    pub fresh: String,
    pub schema: Option<Value>,
    /// The mode the run starts in, as the context states it.
    pub mode: Mode,
    /// Whether there is a terminal to ask the person on.
    pub terminal: bool,
    /// Whether `ask` is offered: in a session, not in `exec`, where one task runs unattended.
    pub asks: bool,
    pub history: Sessions,
    /// The session the tasks are recorded in: new, or resumed.
    pub session: Session,
    /// The interface a session at a terminal is shown in.
    pub ui: Option<Ui>,
    /// Whether the browser and terminal windows show by themselves (`windows::shown`).
    pub shown: bool,
    /// The person's terminal colours, which terminal windows take.
    #[cfg(unix)]
    pub theme: Option<tools::shell::Theme>,
}

/// What every lane of a session shares: the options, the workspace, the browser's page (one
/// action at a time), the session store, and the fresh context, the same bytes for each lane so
/// the prompt cache is shared too.
pub struct Shared {
    pub options: RunOptions,
    pub workspace: Workspace,
    pub page: Option<Page>,
    /// The browser the terminal windows open in, beside the page.
    pub browser: Option<Browser>,
    /// Whether the windows show by themselves (`windows::shown`).
    pub shown: bool,
    /// The person's terminal colours, which terminal windows take.
    #[cfg(unix)]
    pub theme: Option<tools::shell::Theme>,
    /// One browser action at a time, every lane's and every agent's.
    pub browser_lock: crate::kinds::BrowserLocks,
    pub history: Sessions,
    /// The context with no earlier session: what a new lane starts with.
    pub fresh: String,
    /// The mode, models and terminal a new lane starts with.
    pub started: Mode,
    pub choice: Choice,
    pub terminal: bool,
    pub asks: bool,
    /// How many lanes are in a task now.
    pub working: std::sync::atomic::AtomicUsize,
    /// Every lane's queue, for what the window says.
    pub lanes: Arc<crate::arrivals::Lanes>,
    /// Terminal sessions' ids, shared by the lanes: an id names one session in the one window.
    pub terminal_ids: Arc<std::sync::atomic::AtomicU32>,
}

/// What one lane is built from beside what it shares.
pub struct Seat {
    pub context: String,
    pub session: Session,
    pub ui: Option<Ui>,
    pub choice: Choice,
    pub mode: Mode,
    pub schema: Option<Value>,
}

impl Setup {
    /// What the lanes share, with the page on `browser`, and the first lane's seat.
    pub fn split(self, browser: Option<Browser>) -> (Shared, Seat) {
        let shown = self.shown;
        let lanes = Arc::new(crate::arrivals::Lanes::default());
        // What the person does in the window reaches the lanes' runs.
        if let Some(browser) = &browser {
            let told = lanes.clone();
            browser.listen(move |tab, what| told.post(tab, what));
        }
        let shared = Shared {
            page: browser.clone().map(|browser| Page::new(browser, !shown)),
            browser,
            shown,
            #[cfg(unix)]
            theme: self.theme,
            browser_lock: Default::default(),
            options: self.options,
            workspace: self.workspace,
            history: self.history,
            fresh: self.fresh,
            started: self.mode,
            choice: self.choice.clone(),
            terminal: self.terminal,
            asks: self.asks,
            working: Default::default(),
            lanes,
            terminal_ids: Arc::default(),
        };
        let seat = Seat {
            context: self.context,
            session: self.session,
            ui: self.ui,
            choice: self.choice,
            mode: self.mode,
            schema: self.schema,
        };
        (shared, seat)
    }
}

impl Shared {
    /// The seat of a new lane, recorded in `session`: the fresh context, as the session began.
    pub fn seat(&self, mut session: Session, ui: Option<Ui>) -> Seat {
        if session.workspace.is_empty() {
            session.workspace = self.workspace.root().display().to_string();
        }
        Seat {
            context: self.fresh.clone(),
            session,
            ui,
            choice: self.choice.clone(),
            mode: self.started,
            schema: None,
        }
    }
}
