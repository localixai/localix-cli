//! One goal from the command line: the decision loop picks a tool and drives it, the expectations
//! are checked on what the tool shows at the end, and the run is reported. Tools and the decision
//! model arrive through the core's ports; this module names no technology.

use std::sync::Mutex;
use std::time::Instant;

use agent::model::Decider;
use agent::tool::Observation;
use decision_loop::decision_loop::DecisionLoop;
use decision_loop::expect::{self, Verdict};
use decision_loop::goal::Stop;
use decision_loop::meter::{Meter, MeteredDecider, Spend};
use decision_loop::step::GoalStep;
use decision_loop::toolbox::Toolbox;
use decision_loop::values;

use crate::exit;
use crate::report::{Trace, report, status_name};

pub struct Options {
    pub goal: String,
    /// The decision model's id, for the report.
    pub decision_model: String,
    pub expects: Vec<String>,
    /// Actions for the whole goal.
    pub max_steps: usize,
    pub json: bool,
    /// Print PASS or FAIL instead of the answer (`decision-loop check`).
    pub check: bool,
    pub trace: Option<std::path::PathBuf>,
    /// Save a picture of the final view here.
    pub screenshot: Option<std::path::PathBuf>,
    /// Where an earlier session left off: the first tool resumes there.
    pub resume_at: Option<String>,
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// The decision model said DONE.
    Achieved,
    /// It said BLOCKED, or the loop stopped without progress.
    NotAchieved,
    /// The action budget ran out.
    Exhausted,
    /// The decision model could not be reached.
    Error(String),
}

pub struct RunResult {
    /// The tool the loop picked.
    pub tool: Option<String>,
    /// Where the picture of the final view was saved (`--screenshot`).
    pub screenshot: Option<std::path::PathBuf>,
    pub status: Status,
    pub answer: String,
    pub decision: Spend,
    pub actions: usize,
    pub millis: u64,
}

/// What a run leaves for the session.
pub struct Finished {
    pub code: i32,
    pub tool: Option<String>,
    pub answer: String,
    pub status: &'static str,
    pub url: Option<String>,
}

pub fn run(options: &Options, toolbox: &Toolbox, model: &dyn Decider) -> Finished {
    let meter = Meter::default();
    let decider = MeteredDecider {
        inner: model,
        meter: &meter,
    };
    let trace = Trace {
        started: Instant::now(),
        events: Mutex::new(vec![]),
        quiet: options.json && !options.verbose,
    };
    let on_step = |step: &GoalStep| trace.step(step);
    let values = values::candidates(&options.goal);
    let outcome = DecisionLoop {
        toolbox,
        decider: &decider,
        on_step: &on_step,
        max_actions: options.max_steps,
    }
    .run(&options.goal, &values, options.resume_at.as_deref());
    let tool = outcome.tool.as_deref().and_then(|name| toolbox.get(name));
    let outcome = outcome.run;
    let status = match &outcome.stop {
        // DONE was the model's best operation. How sure it was says little about whether it is
        // right (on quiz pages it was 36-58% when right, 65-72% when one step too far), so the
        // answer says how sure it was and `--expect` is the check.
        Stop::Done { .. } => Status::Achieved,
        Stop::Limit => Status::Exhausted,
        Stop::Unreachable(error) => Status::Error(error.clone()),
        _ => Status::NotAchieved,
    };
    let page = tool
        .as_ref()
        .and_then(|tool| tool.observe().ok())
        .or(outcome.view.clone());
    let content = tool
        .as_ref()
        .and_then(|tool| tool.content().ok())
        .unwrap_or_default();
    let verdicts = expect::check(
        &options.expects,
        &page
            .as_ref()
            .map(|page| final_view(page, &content))
            .unwrap_or_default(),
        &decider,
    );
    let code = exit_code(&status, &verdicts);
    let screenshot = match (&options.screenshot, &tool) {
        (Some(path), Some(tool)) => crate::chat::save_capture(tool.as_ref(), path)
            .inspect_err(|error| eprintln!("decision-loop: no screenshot: {error}"))
            .ok(),
        _ => None,
    };
    let shown = page
        .as_ref()
        .map(|page| {
            let text: String = page.text.chars().take(600).collect();
            format!("{} — {}\n{text}", page.title, page.location)
        })
        .unwrap_or_default();
    let result = RunResult {
        tool: tool.as_ref().map(|tool| tool.name().to_string()),
        screenshot,
        status,
        answer: format!("Stopped: {}.\n{shown}", outcome.stop),
        decision: meter.spend(),
        actions: outcome.steps.len(),
        millis: trace.started.elapsed().as_millis() as u64,
    };
    report(options, &result, &verdicts, page.as_ref(), code, &trace);
    Finished {
        code,
        tool: result.tool.clone(),
        status: status_name(&result.status),
        url: page.map(|page| page.location),
        answer: result.answer,
    }
}

/// What an expectation is judged on: what the tool shows, then all it can read.
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

fn exit_code(status: &Status, verdicts: &[Verdict]) -> i32 {
    match status {
        Status::Error(_) => exit::ENVIRONMENT,
        Status::Exhausted => exit::EXHAUSTED,
        Status::NotAchieved => exit::FAILED,
        _ if verdicts.iter().any(|verdict| !verdict.passed) => exit::FAILED,
        _ => exit::OK,
    }
}
