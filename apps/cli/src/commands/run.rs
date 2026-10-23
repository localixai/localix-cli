//! A run, as a session at the terminal or one `exec` task: all is checked and read on the main
//! thread first (workspace, schema, key, prompt), then the session is composed, with the browser's
//! event loop on the main thread where a window can be shown. Each task is run, reported, recorded
//! and its terminal sessions closed; Ctrl-C or closing the window stops it (see `interrupt`), not
//! the process.

use std::io::IsTerminal;
use std::path::Path;
use std::time::{Duration, Instant};

use contracts::history::{History, Session, Turn};
use contracts::tool::Image;
use contracts::view::View;
use serde_json::Value;
use tools::workspace::Workspace;

use crate::attach;
use crate::choice;
use crate::cli::RunOptions;
use crate::commands::{repl, sessions};
use crate::compose::{APP, Rig, Seat, Setup, Shared, compose, lane};
use crate::context::{self, Environment};
use crate::desk;
use crate::error::Failure;
use crate::exit;
use crate::gate::Tty;
use crate::input::{self, Prompt};
use crate::interrupt;
use crate::mode::{self, Mode};
use crate::output::{Start, Summary};
use crate::tui;
use crate::windows;

pub use super::ended::{Ended, since};

/// The workspace `options` name (`-C`, else the current directory), with llx's config (keys,
/// tokens) and sessions off limits to the tools: a prompt injection must not read them.
pub fn root(options: &RunOptions) -> Result<Workspace, Failure> {
    let root = options.cd.as_deref().unwrap_or(Path::new("."));
    let workspace = Workspace::new(root)
        .map_err(|error| Failure::Usage(format!("-C {}: {error}", root.display())))?;
    if !workspace.root().is_dir() {
        let why = format!("-C {}: not a directory", root.display());
        return Err(Failure::Usage(why));
    }
    let private = provider_kit::store::dir(APP).into_iter();
    Ok(workspace.denying(private.chain([sessions::store().dir])))
}

/// How a run takes its tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A session at the terminal: the prompt, when given, is its first message.
    Session,
    /// One task, then exit (`exec`, -p).
    Exec,
}

/// What a run does once composed.
#[derive(Debug, PartialEq, Eq)]
pub enum Work {
    /// One task, then exit.
    One(String),
    /// A session in the interface, after a first message when there is one.
    Session(Option<String>),
}

/// What a session needs that `exec` does not, as a usage error.
const SESSION_TERMINAL: &str = "a session needs a terminal on stdin: run one task with \
`llx exec \"<task>\"` (or -p), or one more in a saved session with `llx exec resume`";
const NO_INTERFACE: &str = "a session's interface cannot start at this terminal";
const RUN_EXEC: &str = "run one task with `llx exec \"<task>\"` (or -p)";
const ONE_TASK: &str = "--json, --schema and -o answer one task: run it with `llx exec` (or -p)";
const EXEC_PROMPT: &str = "exec runs one task: give it as the prompt, or `-` to read it from stdin";

/// The work `kind` asks of `prompt`; stdin is read only for one task (a session's is the
/// terminal its interface reads).
fn work(prompt: Option<&str>, kind: Kind) -> Result<Work, Failure> {
    match kind {
        Kind::Exec => match input::prompt(prompt)? {
            Prompt::One(task) => Ok(Work::One(task)),
            Prompt::Line => Err(Failure::Usage(EXEC_PROMPT.into())),
        },
        Kind::Session => Ok(Work::Session(prompt.map(str::to_string))),
    }
}

/// Whether a session can run: at a terminal, with no option that answers one task.
pub fn session_allowed(
    prompt: Option<&str>,
    options: &RunOptions,
    terminal: bool,
) -> Result<(), Failure> {
    if options.json || options.schema.is_some() || options.output_last_message.is_some() {
        return Err(Failure::Usage(ONE_TASK.into()));
    }
    if !terminal || prompt == Some("-") {
        return Err(Failure::Usage(SESSION_TERMINAL.into()));
    }
    Ok(())
}

/// Run `prompt` with `options` as `kind`, in `resumed` when a session goes on.
pub fn run(
    prompt: Option<&str>,
    options: RunOptions,
    resumed: Option<Session>,
    kind: Kind,
) -> Result<i32, Failure> {
    let workspace = root(&options)?;
    let schema = options.schema.as_deref().map(input::schema).transpose()?;
    // A picture asked for that cannot be attached stops the run before a model is paid for.
    for path in &options.images {
        attach::read(path).map_err(|why| Failure::Usage(format!("--image {why}")))?;
    }
    if kind == Kind::Session {
        session_allowed(prompt, &options, std::io::stdin().is_terminal())?;
    }
    let choice = choice::resolve(&options.models, choice::saved())?;
    // A session starts signed out too, so that /login can sign in; one task cannot.
    if kind == Kind::Exec && choice.provider.signed_in().is_none() {
        return Err(Failure::SignedOut(choice.provider.how_to_sign_in()));
    }
    let work = work(prompt, kind)?;
    let mode = Mode::choose(options.mode, options.yes);
    let (terminal, asks) = (Tty::available(), kind == Kind::Session);
    let browser = tools::browser::has_display();
    let remote = windows::remote(|name| std::env::var_os(name).is_some());
    let shown = windows::shown(&options, asks, mode, remote);
    // The person's colours are asked only where terminal windows will show them.
    #[cfg(unix)]
    let windowed = browser && shown;
    let environment = Environment {
        root: workspace.root(),
        mode: &mode::describe(mode, terminal, asks),
        browser,
        now: session::now(),
    };
    let context = context::gather(&environment, resumed.as_ref());
    let fresh = context::gather(&environment, None);
    let history = sessions::store();
    let mut session = resumed.unwrap_or_else(|| history.start());
    if session.workspace.is_empty() {
        session.workspace = workspace.root().display().to_string();
    }
    let mut setup = Setup {
        options,
        choice,
        workspace,
        context,
        fresh,
        schema,
        mode,
        terminal,
        asks,
        history,
        session,
        ui: None,
        shown,
        #[cfg(unix)]
        theme: None,
    };
    // A session takes the terminal for its interface; one that cannot draw there (no size to
    // draw at) is a usage error, as having no terminal is.
    let running = match &work {
        Work::Session(_) => {
            // Before the interface reads the keys, which would take the terminal's answers.
            #[cfg(unix)]
            if windowed {
                setup.theme = crate::colours::ask();
            }
            // xterm.js for the terminal windows, once: until it is kept they show a grid.
            #[cfg(unix)]
            if crate::xterm::wanted(true, windowed, |name| std::env::var(name).ok()) {
                crate::xterm::prefetch();
            }
            let running = tui::start(repl::commands(), setup.workspace.root());
            let why = |error| Failure::Usage(format!("{NO_INTERFACE} ({error}): {RUN_EXEC}"));
            Some(running.map_err(why)?)
        }
        Work::One(_) => None,
    };
    // A newer model catalog, for the next session: nothing here waits on it.
    if running.is_some() {
        crate::providers::refresh_catalog();
    }
    let mut running = running;
    let work = move |browser| {
        let code = match (work, running.as_mut()) {
            // Each lane of the session is a session of its own, on its own thread.
            (Work::Session(first), Some(interface)) => {
                let ui = interface.ui.clone();
                let requests = interface.requests.take().expect("the desk's requests");
                let (shared, seat) = setup.split(browser);
                let seat = Seat {
                    ui: Some(ui.clone()),
                    ..seat
                };
                let start = |shared: &Shared, seat, work: desk::Work| lane(shared, seat, work);
                // Lanes that do not stop in time: the terminal goes back, and the process ends.
                let quit = || {
                    tui::restore();
                    interrupt::quit_now();
                };
                desk::run(&shared, &ui, requests, seat, first, &start, &quit)
            }
            (Work::Session(_), None) => unreachable!("a session has its interface"),
            (Work::One(task), _) => compose(setup, browser, |rig| {
                rig.attached = rig.options.images.clone();
                rig.task(&task)
            }),
        };
        drop(running);
        code
    };
    interrupt::install();
    if !browser {
        return Ok(work(None));
    }
    tools::browser::host(interrupt::window_closed, move |b| work(Some(b)))
}

impl Rig<'_> {
    /// Run one task in the session: report it, record it, and return its exit code.
    pub fn task(&mut self, goal: &str) -> i32 {
        let started = Instant::now();
        let before = self.meter.spend();
        let workspace = self.workspace.root().display().to_string();
        self.output.started(&Start {
            goal,
            session: &self.session.id,
            model: self.choice.model(),
            decision_model: &self.options.models.decision_model,
            workspace: &workspace,
        });
        let attached = self.pictures();
        self.brake.arm(Duration::from_secs(self.options.timeout));
        let _working = Working::of(&self.shared.working);
        self.interrupt.begin();
        self.jobs.begin();
        let at = self.conversation.as_ref().map_or(0, |c| c.messages().len());
        self.checkpoints.begin(goal, at);
        // The model may have changed since the last task: its window decides when to compact.
        self.agent.window = self.choice.selection.model().context;
        self.window
            .store(self.agent.window, std::sync::atomic::Ordering::SeqCst);
        let mut run = match self.told_of_mode() {
            Some(before) => self.agent.follow_up_with(before, goal, &attached),
            None => self.agent.run_with(goal, &attached),
        };
        self.agents.settle();
        self.jobs.end(self.jobs_kit);
        run.spend.absorb(&self.agents.take_spent());
        self.spent.absorb(&run.spend);
        self.context_left(run.used);
        let halt = self.brake.halted();
        self.sessions.close_all();
        self.interrupt.end();
        self.conversation = Some(run.conversation);
        let ended = Ended::of(&run.status, run.data.as_ref(), halt, self.options.timeout);
        let decision = since(&self.meter.spend(), &before);
        self.record(goal, &run.answer, ended.status);
        // `-o` first, so `exit_code` in the report is the code the process ends with.
        let (code, error) = match self.write_last(&run.answer, run.data.as_ref()) {
            Ok(()) => (ended.code, ended.error),
            Err(written) => {
                let error = match ended.error {
                    Some(error) => format!("{error}; {written}"),
                    None => written,
                };
                (exit::ENVIRONMENT, Some(error))
            }
        };
        let summary = Summary {
            status: ended.status,
            code,
            answer: &run.answer,
            data: run.data.as_ref(),
            error: error.as_deref(),
            agent: &run.spend,
            decision: &decision,
            millis: started.elapsed().as_millis() as u64,
            session: &self.session.id,
        };
        self.output.completed(&summary);
        if self.output.closed() { exit::OK } else { code }
    }

    /// The pictures attached to this task, read; one that cannot be is left out, and said.
    fn pictures(&mut self) -> Vec<(String, Image)> {
        let mut out = vec![];
        for path in std::mem::take(&mut self.attached) {
            match attach::read(&path) {
                Ok(picture) => out.push((picture.name, picture.image)),
                Err(why) => self.output.note(&format!("llx: not attached: {why}\n")),
            }
        }
        out
    }

    /// `-o FILE`: the final answer, or with `--schema` the data. With `--schema` and no data that
    /// matched, the file is left empty: prose where data is expected would break the reader.
    fn write_last(&self, answer: &str, data: Option<&Result<Value, String>>) -> Result<(), String> {
        let Some(path) = &self.options.output_last_message else {
            return Ok(());
        };
        let last = match (data, self.agent.schema.is_some()) {
            (Some(Ok(data)), _) => format!("{data}\n"),
            (_, true) => String::new(),
            (_, false) => format!("{}\n", answer.trim()),
        };
        std::fs::write(path, last).map_err(|error| format!("-o {}: {error}", path.display()))
    }

    /// Add the task to the session, with where the browser was left, and save it.
    /// How much of the model's context window is left, for the interface: `used` tokens of it.
    fn context_left(&self, used: u64) {
        let ceiling = self.agent.strategy.ceiling(self.agent.window);
        if let (Some(ui), true) = (&self.ui, self.agent.window > 0) {
            let left = 100u64.saturating_sub(used * 100 / ceiling.max(1));
            ui.send(crate::tui::event::Event::Context(left as u8));
        }
    }

    fn record(&mut self, goal: &str, answer: &str, status: &str) {
        let location = (self.shared.page.as_ref())
            .filter(|page| page.is_open())
            .and_then(|page| page.observe().ok())
            .map(|view| view.location);
        let turn = Turn {
            goal: goal.to_string(),
            answer: answer.to_string(),
            status: status.to_string(),
            at: session::now(),
        };
        self.session.record(turn, location.as_deref());
        if let Err(error) = self.history.save(&self.session) {
            self.output
                .note(&format!("llx: the session was not saved: {error}\n"));
        }
    }
}

/// A lane in a task, counted while it lasts (and even if it unwinds), for `/rewind`'s warning.
struct Working<'a>(&'a std::sync::atomic::AtomicUsize);

impl<'a> Working<'a> {
    fn of(count: &'a std::sync::atomic::AtomicUsize) -> Self {
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self(count)
    }
}

impl Drop for Working<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
