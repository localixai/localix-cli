//! One goal from the command line. The agent reaches it with the browser, whose hands are the
//! decision loop; with `--no-llm` the hands take the whole goal themselves. Then the expectations
//! are checked on the final page and the run is reported.

use std::sync::Mutex;
use std::time::Instant;

use agent::agent::{Agent, Spend as ChatSpend, Status as AgentStatus};
use agent::conversation::Conversation;
use contracts::chat::{Chat, ToolCall};
use contracts::model::Decider;
use contracts::view::{Observation, View};
use decision_loop::decision_loop::DecisionLoop;
use decision_loop::expect::{self, Verdict};
use decision_loop::goal::Stop;
use decision_loop::meter::{Meter, MeteredDecider, Spend};
use decision_loop::step::GoalStep;
use decision_loop::values;
use serde_json::Value;
use tools::browser::{BrowserTool, Page, SEARCH_PAGE, url_in};

use crate::exit;
use crate::report::{Trace, report, status_name};

pub struct Options {
    pub goal: String,
    /// The models' ids, for the report.
    pub chat_model: String,
    pub decision_model: String,
    /// The hands alone, without the chat model.
    pub no_llm: bool,
    pub schema: Option<Value>,
    pub expects: Vec<String>,
    pub max_turns: usize,
    /// Actions for one task of the hands (with `--no-llm`, for the whole goal).
    pub max_steps: usize,
    pub json: bool,
    /// Print PASS or FAIL instead of the answer (`lxsurf check`).
    pub check: bool,
    pub trace: Option<std::path::PathBuf>,
    /// Save a picture of the final page here.
    pub screenshot: Option<std::path::PathBuf>,
    /// Where an earlier session left off: the window opens there.
    pub resume_at: Option<String>,
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Achieved,
    NotAchieved,
    /// The agent answered without saying whether the goal was reached.
    Unstated,
    /// Turns, actions or time ran out before an outcome.
    Exhausted,
    /// A model could not be reached.
    Error(String),
}

pub struct RunResult {
    pub status: Status,
    pub answer: String,
    /// `--schema`: the data, or why none matched.
    pub data: Option<Result<Value, String>>,
    /// Where the picture of the final page was saved (`--screenshot`).
    pub screenshot: Option<std::path::PathBuf>,
    pub chat: ChatSpend,
    pub decision: Spend,
    pub actions: usize,
    pub millis: u64,
}

/// What a run leaves for the session.
pub struct Finished {
    pub code: i32,
    pub answer: String,
    pub status: &'static str,
    pub url: Option<String>,
}

/// The models a run uses.
pub struct Models<'a> {
    pub chat: &'a dyn Chat,
    pub decider: &'a dyn Decider,
}

pub fn run(
    options: &Options,
    page: &Page,
    models: &Models,
    conversation: &mut Option<Conversation>,
) -> Finished {
    let meter = Meter::default();
    let decider = MeteredDecider {
        inner: models.decider,
        meter: &meter,
    };
    let trace = Trace {
        started: Instant::now(),
        events: Mutex::new(vec![]),
        quiet: options.json && !options.verbose,
    };
    let actions = Mutex::new(0);
    let on_step = |step: &GoalStep| {
        *actions.lock().unwrap() += 1;
        trace.step(step);
    };
    let hands = DecisionLoop {
        decider: &decider,
        on_step: &on_step,
        max_actions: options.max_steps,
    };
    if let Some(at) = &options.resume_at {
        let _ = page.open(at, &options.goal);
    }
    let (status, answer, data, chat) = if options.no_llm {
        let (status, answer) = hands_alone(options, page, &hands);
        (status, answer, None, ChatSpend::default())
    } else {
        let browser = BrowserTool {
            page,
            hands: &hands,
        };
        let on_call = |call: &ToolCall| trace.call(&call.name, &call.arguments);
        let agent = Agent {
            chat: models.chat,
            tools: &[&browser],
            max_turns: options.max_turns,
            on_call: &on_call,
            schema: options.schema.as_ref(),
        };
        let done = match conversation.take() {
            Some(before) => agent.follow_up(before, &options.goal),
            None => agent.run(&options.goal),
        };
        *conversation = Some(done.conversation);
        let status = match done.status {
            AgentStatus::Achieved => Status::Achieved,
            AgentStatus::NotAchieved => Status::NotAchieved,
            AgentStatus::Unstated => Status::Unstated,
            AgentStatus::Exhausted => Status::Exhausted,
            AgentStatus::Error(error) => Status::Error(error),
        };
        (status, done.answer, done.data, done.spend)
    };
    let view = page.observe().ok();
    let content = page.content().unwrap_or_default();
    let verdicts = expect::check(
        &options.expects,
        &view
            .as_ref()
            .map(|view| final_view(view, &content))
            .unwrap_or_default(),
        &decider,
    );
    let code = exit_code(&status, &verdicts, &data);
    let screenshot = options.screenshot.as_ref().and_then(|path| {
        crate::chat::save_capture(page, path)
            .inspect_err(|error| eprintln!("lxsurf: no screenshot: {error}"))
            .ok()
    });
    let result = RunResult {
        status,
        answer,
        data,
        screenshot,
        chat,
        decision: meter.spend(),
        actions: *actions.lock().unwrap(),
        millis: trace.started.elapsed().as_millis() as u64,
    };
    report(options, &result, &verdicts, view.as_ref(), code, &trace);
    Finished {
        code,
        status: status_name(&result.status),
        url: view.map(|view| view.location),
        answer: result.answer,
    }
}

/// `--no-llm`: the hands take the whole goal, typing only the goal's own words, from the page it
/// names (or where the window is, or a search page).
fn hands_alone(options: &Options, page: &Page, hands: &DecisionLoop) -> (Status, String) {
    let start =
        url_in(&options.goal).or_else(|| (!page.is_open()).then(|| SEARCH_PAGE.to_string()));
    if let Some(url) = start
        && let Err(error) = page.open(&url, &options.goal)
    {
        return (
            Status::Error(format!("{url} did not open: {error}")),
            String::new(),
        );
    }
    let values = values::candidates(&options.goal);
    let outcome = hands.run(page, &options.goal, &values);
    let status = match &outcome.stop {
        // DONE was the model's best operation. How sure it was says little about whether it is
        // right, so `--expect` is the check.
        Stop::Done { .. } => Status::Achieved,
        Stop::Limit => Status::Exhausted,
        Stop::Unreachable(error) => Status::Error(error.clone()),
        _ => Status::NotAchieved,
    };
    let shown = outcome
        .view
        .map(|view| {
            let text: String = view.text.chars().take(600).collect();
            format!("{} — {}\n{text}", view.title, view.location)
        })
        .unwrap_or_default();
    (status, format!("Stopped: {}.\n{shown}", outcome.stop))
}

/// What an expectation is judged on: what the page shows, then all it can read.
fn final_view(view: &Observation, content: &str) -> String {
    let controls: Vec<String> = view
        .elements
        .iter()
        .take(100)
        .map(|element| element.describe())
        .collect();
    format!(
        "{} — {}\nVisible text:\n{}\nControls: {}\nFull text:\n{content}",
        view.title,
        view.location,
        view.text,
        controls.join(" · ")
    )
}

fn exit_code(status: &Status, verdicts: &[Verdict], data: &Option<Result<Value, String>>) -> i32 {
    match status {
        Status::Error(_) => exit::ENVIRONMENT,
        Status::Exhausted => exit::EXHAUSTED,
        Status::NotAchieved => exit::FAILED,
        _ if matches!(data, Some(Err(_))) => exit::FAILED,
        _ if verdicts.iter().any(|verdict| !verdict.passed) => exit::FAILED,
        _ => exit::OK,
    }
}
