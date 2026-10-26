//! What a toolbox is built from at its client's place, and the `window` tool built from it for
//! each call: the agent's own tool and kinds, on a gate and a stop of that call's (K7).

use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use contracts::tool::{Tool, ToolError};
use decision_loop::decision_loop::DecisionLoop;
use decision_loop::step::GoalStep;
use protocol::{Done, Place, ToolInfo};
use serde_json::Value;
use tools::browser::Page;
use tools::shell::{BashTool, Sessions};
use tools::window::{Act, WindowTab, WindowTool, Windowed};
use tools::workspace::Workspace;

use super::Hosted;
use crate::brake::BrakedDecider;
use crate::gate::Stoppable;
use crate::interrupt::Interrupt;
use crate::kinds::{self, BrowserLocks};
use crate::live::LiveDecider;
use crate::mode::{Mode, ModeCell, ModeGate};
use crate::modes::{BYPASS, Modes};
use crate::output::lock;
use crate::providers::{self, Provider};
use crate::rules::Rules;
use crate::server::{place, wire};
use crate::windows::{self, Tabs};

/// The one tool a tool client calls.
const WINDOW: &str = "window";
/// Actions of the decision model for one goal, as `--max-steps` gives by default.
const MAX_STEPS: usize = 40;

/// A toolbox at its client's place: the workspace, the browser's page, its terminal sessions,
/// the window as the person sees it, and the decision model, with the client's keys.
pub(super) struct Kit {
    workspace: Workspace,
    page: Option<Page>,
    locks: BrowserLocks,
    sessions: Sessions,
    /// Their numbering, which names the sessions running.
    terminals: Arc<AtomicU32>,
    held: Option<Held>,
    decider: LiveDecider,
    /// What commands never see: the providers' keys.
    hidden: Vec<String>,
}

impl Kit {
    pub(super) fn new(place: &Place, hosted: &Hosted) -> Result<Self, String> {
        let workspace = place::workspace(place)?;
        // Over SSH the window would open on a screen the person is not at (`windows::shown`).
        let remote = windows::remote(|name| place.env.iter().any(|(set, _)| set == name));
        let shown = hosted.browser.is_some() && !remote;
        let group = windows::group(&place.cwd, workspace.root());
        let page = (hosted.browser.clone()).map(|browser| Page::new(browser, !shown));
        // What the person sends from its tabs waits here for the client's next call.
        let heard = Arc::new(crate::arrivals::queue(None));
        let held = (hosted.browser.clone().filter(|_| shown)).map(|browser| Held {
            tabs: Tabs::of(browser, &group).owned(&hosted.lanes, &heard),
            opened: Mutex::default(),
            heard,
        });
        // Its terminals' tabs tell no lane what the person does to them: it has none.
        let watch = hosted.browser.clone().map(|browser| {
            let owner = (Arc::clone(&hosted.lanes), Weak::new(), group.clone());
            windows::panels(browser, place::theme(place), owner)
        });
        let terminals = Arc::clone(&hosted.terminals);
        let sessions = windows::terminals(watch, shown).numbered(Arc::clone(&terminals));
        let chosen = place
            .env
            .iter()
            .find(|(name, _)| name == "LLX_DECISION_MODEL");
        let model = (chosen.map(|(_, model)| model.as_str()))
            .filter(|model| !model.is_empty())
            .unwrap_or(providers::DECISION_DEFAULT);
        let decider = LiveDecider::new(providers::decider_in(model, Some(&place.env)));
        let hidden = (Provider::all().iter())
            .flat_map(Provider::hidden)
            .collect();
        Ok(Self {
            workspace,
            page,
            locks: BrowserLocks::default(),
            sessions,
            terminals,
            held,
            decider,
            hidden,
        })
    }

    /// The tool, as its client's model reads it.
    pub(super) fn info(&self) -> Option<ToolInfo> {
        let interrupt = Interrupt::new();
        let spec = self.with_window(&interrupt, |window| Ok(window.spec()));
        spec.ok().map(wire::tool)
    }

    /// Tool `name` called with `arguments`, stopped by `interrupt`; a panic in it is this call's
    /// failure, never the daemon's end (W8).
    pub(super) fn call(&self, name: &str, arguments: &Value, interrupt: &Interrupt) -> Done {
        if name != WINDOW {
            return Done::Failed {
                why: format!("there is no tool {name} here: `{WINDOW}` is the one"),
            };
        }
        let calling = std::panic::AssertUnwindSafe(|| {
            self.with_window(interrupt, |window| window.call(arguments))
        });
        let called = std::panic::catch_unwind(calling)
            .unwrap_or_else(|_| Err(ToolError::Failed("the tool failed doing it".into())));
        if let Some(held) = &self.held
            && arguments.get("action").and_then(Value::as_str) == Some("close")
        {
            held.closed(arguments);
        }
        wire::called(called)
    }

    /// Whether a tab it opened is still open: a terminal session running, or a page.
    pub(super) fn holds(&self) -> bool {
        !tools::shell::running_with(&self.terminals).is_empty()
            || self
                .held
                .as_ref()
                .is_some_and(|held| !held.pages().is_empty())
    }

    /// Its terminal sessions ended, and the pages its client opened in the window and left open
    /// closed.
    pub(super) fn close(&self) {
        self.sessions.close_all();
        let Some(held) = &self.held else {
            return;
        };
        for page in held.pages() {
            let _ = held.tabs.act(Act::Close, &page);
        }
        lock(&held.opened).clear();
    }

    /// `work` with the `window` tool as the agent has it, on a gate that allows (the client
    /// approved the call, K7) until `interrupt` stops the call.
    fn with_window<T>(
        &self,
        interrupt: &Interrupt,
        work: impl FnOnce(&WindowTool) -> Result<T, ToolError>,
    ) -> Result<T, ToolError> {
        let stopped = || interrupt.stopped();
        // Built in, so always there; a mode with no rules would only refuse.
        let bypass = Modes::built_in().named(BYPASS);
        let mode = ModeCell::new(bypass.unwrap_or_else(|| Mode::new(BYPASS, "", Rules::default())));
        let waited = |_: Duration| {};
        let approver = ModeGate::asking(&mode, None, &waited);
        let gate = Stoppable {
            inner: &approver,
            stopped: &stopped,
        };
        let hidden = || self.hidden.clone();
        let bash = BashTool {
            workspace: &self.workspace,
            gate: &gate,
            hidden: &hidden,
            stop: &stopped,
            sessions: &self.sessions,
        };
        let decider = BrakedDecider {
            inner: &self.decider,
            interrupt,
        };
        let decider = crate::hands::Traced::new(&decider);
        let on_step = |_: &GoalStep| {};
        let hands = DecisionLoop {
            decider: &decider,
            on_step: &on_step,
            pause: &crate::hands::pause,
            max_actions: MAX_STEPS,
        };
        let parts = kinds::Parts {
            bash: &bash,
            sessions: &self.sessions,
            stop: &stopped,
            page: self.page.as_ref(),
            locks: &self.locks,
            windowed: self.held.as_ref().map(|held| held as &dyn Windowed),
            hands: &hands,
            gate: &gate,
        };
        kinds::window(&parts, |window| match window {
            Some(window) => work(window),
            None => Err(ToolError::Failed("no kind of tab here".into())),
        })
    }
}

/// The window as the client's calls see it, and the tabs they opened there.
struct Held {
    tabs: Tabs,
    opened: Mutex<Vec<String>>,
    /// What the person did to its tabs, and sent from them, until a call takes it.
    heard: Arc<queue::Queue>,
}

impl Held {
    /// The pages its client opened that are still open, of its project: an id the client closed
    /// may name another's now.
    fn pages(&self) -> Vec<String> {
        let opened = lock(&self.opened).clone();
        let listed = self.tabs.tabs().unwrap_or_default();
        let own = listed
            .into_iter()
            .filter(|tab| tab.group == self.tabs.group);
        own.map(|tab| tab.id)
            .filter(|id| id.starts_with('w') && opened.contains(id))
            .collect()
    }

    /// The tabs a `close` call named are no longer the client's to close.
    fn closed(&self, arguments: &Value) {
        let one = arguments.get("tab").and_then(Value::as_str);
        let several = arguments.get("tabs").and_then(Value::as_array);
        let named: Vec<&str> = (several.into_iter().flatten())
            .filter_map(Value::as_str)
            .chain(one)
            .collect();
        lock(&self.opened).retain(|tab| !named.contains(&tab.as_str()));
    }
}

impl Windowed for Held {
    fn tabs(&self) -> Option<Vec<WindowTab>> {
        self.tabs.tabs()
    }

    fn act(&self, act: Act, tab: &str) -> Result<(), String> {
        self.tabs.act(act, tab)
    }

    fn resize(&self, width: u32, height: u32) -> Result<(), String> {
        self.tabs.resize(width, height)
    }

    fn opened(&self, tab: &str) {
        self.tabs.opened(tab);
        lock(&self.opened).push(tab.to_string());
    }

    fn said(&self) -> Option<String> {
        // Taken as a running task takes it: everything waiting, as the inbox renders it.
        self.heard.begin();
        let taken = self.heard.take_joining();
        self.heard.end();
        let said: Vec<String> = taken.iter().map(queue::Entry::render).collect();
        (!said.is_empty()).then(|| said.join("\n\n"))
    }
}
