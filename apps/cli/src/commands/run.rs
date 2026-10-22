//! A run: everything checked and read on the main thread first (the workspace, the schema, the
//! key, the prompt), then the session is composed, with the browser's event loop on the main
//! thread where a window can be shown. Each task is run, reported, and recorded in the session;
//! Ctrl-C or closing the window stops the task (see `interrupt`), not the process.

use std::path::Path;
use std::time::{Duration, Instant};

use agent::agent::Status;
use contracts::history::{History, Session, Turn};
use contracts::view::View;
use decision_loop::meter::Spend;
use openrouter::auth;
use serde_json::Value;
use tools::workspace::Workspace;

use crate::brake::Halt;
use crate::cli::RunOptions;
use crate::commands::{repl, sessions};
use crate::compose::{APP, Rig, Setup, compose};
use crate::context::{self, Environment};
use crate::error::Failure;
use crate::exit;
use crate::gate::Approvals;
use crate::input::{self, Prompt};
use crate::interrupt;
use crate::output::{Start, Summary};

/// The workspace `options` name (`-C`, else the current directory), with lxsurf's own key and
/// sessions off limits to the tools: a prompt injection must not read them into the conversation.
pub fn root(options: &RunOptions) -> Result<Workspace, Failure> {
    let root = options.cd.as_deref().unwrap_or(Path::new("."));
    let workspace = Workspace::new(root)
        .map_err(|error| Failure::Usage(format!("-C {}: {error}", root.display())))?;
    if !workspace.root().is_dir() {
        return Err(Failure::Usage(format!(
            "-C {}: not a directory",
            root.display()
        )));
    }
    let private = auth::credentials_path(APP).into_iter();
    Ok(workspace.denying(private.chain([sessions::store().dir])))
}

/// Run `prompt` (or the prompt line) with `options`, in `resumed` when a session goes on.
pub fn run(
    prompt: Option<&str>,
    options: RunOptions,
    resumed: Option<Session>,
) -> Result<i32, Failure> {
    let workspace = root(&options)?;
    let schema = options.schema.as_deref().map(input::schema).transpose()?;
    let (key, _) = auth::key(APP).ok_or(Failure::SignedOut)?;
    let prompt = input::prompt(prompt)?;
    if prompt == Prompt::Line && (options.json || schema.is_some()) {
        return Err(Failure::Usage(
            "--json and --schema answer one task: give it as the prompt".into(),
        ));
    }
    let approvals = Approvals::choose(options.yes);
    let browser = tools::browser::has_display();
    let environment = Environment {
        root: workspace.root(),
        approvals: approvals.describe(),
        browser,
        now: session::now(),
    };
    let context = context::gather(&environment, resumed.as_ref());
    let history = sessions::store();
    let mut session = resumed.unwrap_or_else(|| history.start());
    if session.workspace.is_empty() {
        session.workspace = workspace.root().display().to_string();
    }
    let headless = !options.show_browser;
    let setup = Setup {
        options,
        key,
        workspace,
        context,
        schema,
        approvals,
        history,
        session,
    };
    let work = move |browser| {
        compose(setup, browser, |rig| match prompt {
            Prompt::One(task) => rig.task(&task),
            Prompt::Line => repl::run(rig),
        })
    };
    interrupt::install();
    if !browser {
        return Ok(work(None));
    }
    tools::browser::host(headless, interrupt::window_closed, move |browser| {
        work(Some(browser))
    })
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
            model: &self.options.models.model,
            decision_model: &self.options.models.decision_model,
            workspace: &workspace,
        });
        self.brake.arm(Duration::from_secs(self.options.timeout));
        self.interrupt.begin();
        let run = match self.conversation.take() {
            Some(before) => self.agent.follow_up(before, goal),
            None => self.agent.run(goal),
        };
        let halt = self.brake.halted();
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
    fn record(&mut self, goal: &str, answer: &str, status: &str) {
        let location = self
            .page
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
                .note(&format!("lxsurf: the session was not saved: {error}\n"));
        }
    }
}

/// How a task ended, for the report and the exit code.
#[derive(Debug, PartialEq)]
pub struct Ended {
    pub status: &'static str,
    pub code: i32,
    pub error: Option<String>,
}

impl Ended {
    pub fn of(
        status: &Status,
        data: Option<&Result<Value, String>>,
        halt: Option<Halt>,
        timeout: u64,
    ) -> Self {
        let ended = |status, code, error: Option<String>| Self {
            status,
            code,
            error,
        };
        match (halt, status) {
            (Some(Halt::TimedOut), _) => ended(
                "exhausted",
                exit::EXHAUSTED,
                Some(format!(
                    "the time limit ran out ({timeout} s; raise it with --timeout)"
                )),
            ),
            (Some(Halt::Closed), _) => ended("interrupted", exit::OK, None),
            (Some(Halt::Interrupted), _) => ended("interrupted", exit::INTERRUPTED, None),
            (None, Status::Error(error)) => ended("error", exit::ENVIRONMENT, Some(error.clone())),
            (None, Status::Exhausted) => ended(
                "exhausted",
                exit::EXHAUSTED,
                Some("out of turns before an answer (raise them with --max-turns)".into()),
            ),
            (None, Status::NotAchieved) => ended("not_achieved", exit::FAILED, None),
            (None, _) if matches!(data, Some(Err(_))) => ended("not_achieved", exit::FAILED, None),
            (None, Status::Achieved) => ended("achieved", exit::OK, None),
            (None, Status::Unstated) => ended("unstated", exit::OK, None),
        }
    }
}

/// What the decision model spent since `before`: the meter counts over the whole process.
pub fn since(now: &Spend, before: &Spend) -> Spend {
    Spend {
        model: now.model.clone(),
        calls: now.calls - before.calls,
        input_tokens: now.input_tokens - before.input_tokens,
        output_tokens: now.output_tokens - before.output_tokens,
        cost: now.cost - before.cost,
        millis: now.millis - before.millis,
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
