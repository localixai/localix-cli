//! What a session is built from ([`Setup`]), what its lanes share ([`Shared`]), and what one
//! lane is built from beside it ([`Seat`]).

use contracts::history::Session;
use serde_json::Value;
use session::Sessions;
use tools::workspace::Workspace;

use crate::choice::Choice;
use crate::cli::RunOptions;
use crate::mode::Mode;
use crate::modes::Modes;
use crate::rules::Rules;
use crate::server::Ui;

/// Everything a session is built from, decided before any lane starts (so a bad argument
/// still fails fast).
pub(crate) struct Setup {
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
    /// Every mode a session can switch to.
    pub modes: Modes,
    /// The command line's `--allow` and `--deny`, beside each mode's own.
    pub rules: Rules,
    /// Whether there is a terminal to ask the person on.
    pub terminal: bool,
    /// Whether `ask` is offered: in a session, not in `exec`, where one task runs unattended.
    pub asks: bool,
    pub history: Sessions,
    /// The session the tasks are recorded in: new, or resumed.
    pub session: Session,
    /// The interface a session at a terminal is shown in.
    pub ui: Option<Ui>,
}

/// What every lane of a session shares: the options, the workspace, the session store, and the
/// fresh context, the same bytes for each lane so the prompt cache is shared too.
pub(crate) struct Shared {
    pub options: RunOptions,
    pub workspace: Workspace,
    pub history: Sessions,
    /// The context with no earlier session: what a new lane starts with.
    pub fresh: String,
    /// The mode, models and terminal a new lane starts with.
    pub started: Mode,
    pub modes: Modes,
    pub rules: Rules,
    pub choice: Choice,
    pub terminal: bool,
    pub asks: bool,
    /// The person at this interface, asked one question at a time (`gate::one_question`): each
    /// terminal of the daemon is a person of its own.
    pub asking: std::sync::Mutex<()>,
}

/// What one lane is built from beside what it shares.
pub(crate) struct Seat {
    pub context: String,
    pub session: Session,
    pub ui: Option<Ui>,
    pub choice: Choice,
    pub mode: Mode,
    pub schema: Option<Value>,
}

impl Setup {
    /// What the lanes share, and the first lane's seat.
    pub(crate) fn split(self) -> (Shared, Seat) {
        let shared = Shared {
            options: self.options,
            workspace: self.workspace,
            history: self.history,
            fresh: self.fresh,
            started: self.mode.clone(),
            modes: self.modes,
            rules: self.rules,
            choice: self.choice.clone(),
            terminal: self.terminal,
            asks: self.asks,
            asking: Default::default(),
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
    pub(crate) fn seat(&self, mut session: Session, ui: Option<Ui>) -> Seat {
        if session.workspace.is_empty() {
            session.workspace = self.workspace.root().display().to_string();
        }
        Seat {
            context: self.fresh.clone(),
            session,
            ui,
            choice: self.choice.clone(),
            mode: self.started.clone(),
            schema: None,
        }
    }
}
