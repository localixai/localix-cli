//! One run from the command line: the window is open; plan and surf, check expectations, report.

use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Value, json};

use crate::browser::{Driver, Observation};
use crate::decision::{Answer, Decider, Question, SystemOne};
use crate::exit;
use crate::expect::{self, Verdict};
use crate::goal::{GoalRun, Stop};
use crate::goal_ask::GoalStep;
use crate::meter::{Meter, MeteredDecider, MeteredPlanner};
use crate::openrouter::OpenRouter;
use crate::orchestrator::{Orchestrator, RunResult, Status, Usage};
use crate::planner::ChatModel;
use crate::report::{Trace, report};
use crate::tools::{Tools, describe_page};
use crate::values;

pub struct Options {
    pub goal: String,
    pub key: String,
    pub model: String,
    pub decision_model: String,
    pub no_llm: bool,
    pub expects: Vec<String>,
    pub max_turns: usize,
    pub max_steps: usize,
    pub json: bool,
    /// Print PASS or FAIL instead of the answer (`lxsurf check`).
    pub check: bool,
    pub trace: Option<std::path::PathBuf>,
    pub verbose: bool,
    /// `--schema`: print the result as JSON matching it.
    pub schema: Option<Value>,
}

pub fn run(options: &Options, driver: &dyn Driver) -> i32 {
    let client = OpenRouter::new(options.key.clone());
    let system_one = SystemOne {
        client: &client,
        model: options.decision_model.clone(),
    };
    let chat = ChatModel {
        client: &client,
        model: options.model.clone(),
    };
    let (planner_meter, decision_meter) = (Meter::default(), Meter::default());
    let decider = MeteredDecider {
        inner: &system_one,
        meter: &decision_meter,
    };
    let planner = MeteredPlanner {
        inner: &chat,
        meter: &planner_meter,
    };
    let trace = Trace {
        started: Instant::now(),
        events: Mutex::new(vec![]),
        quiet: options.json && !options.verbose,
    };
    let on_step = |step: &GoalStep| trace.step(step);
    let mut result = if options.no_llm {
        surf_alone(options, driver, &decider, &on_step)
    } else {
        let tools = Tools {
            driver,
            decider: &decider,
            on_step: &on_step,
            max_actions: options.max_steps,
        };
        let on_call = |name: &str, args: &Value| trace.call(name, args);
        let orchestrator = Orchestrator {
            planner: &planner,
            tools,
            driver,
            max_turns: options.max_turns,
            on_call: &on_call,
            schema: options.schema.clone(),
        };
        orchestrator.run(&options.goal)
    };
    let page = driver.observe().ok();
    let page_text = page
        .as_ref()
        .map(|page| final_page(driver, page))
        .unwrap_or_default();
    let verdicts = expect::check(
        &options.expects,
        &page_text,
        &decider,
        (!options.no_llm).then_some(&planner as _),
    );
    let status = match &result.status {
        Status::Unstated => stated(&decider, &options.goal, &result.answer),
        other => other.clone(),
    };
    let mut code = exit_code(&status, &verdicts);
    if options.schema.is_some() && result.data.is_none() && !matches!(status, Status::Error(_)) {
        result.data = Some(Err("the run ended without a result".into()));
    }
    if matches!(result.data, Some(Err(_))) && code == exit::OK {
        code = exit::FAILED;
    }
    result.usage.planner = planner_meter.spend();
    result.usage.decision = decision_meter.spend();
    report(
        options,
        &result,
        &status,
        &verdicts,
        page.as_ref(),
        code,
        &trace,
    );
    code
}

/// The decision model alone: one goal, values found in the goal itself, no answer text.
fn surf_alone(
    options: &Options,
    driver: &dyn Driver,
    decider: &dyn Decider,
    on_step: &dyn Fn(&GoalStep),
) -> RunResult {
    let started = Instant::now();
    let values = values::candidates(&options.goal);
    let outcome = GoalRun {
        driver,
        decider,
        on_step,
        max_actions: options.max_steps,
    }
    .run(&options.goal, &values);
    let status = match outcome.stop {
        Stop::Done { probability } if probability >= crate::goal::DONE_CONFIDENCE => {
            Status::Achieved
        }
        Stop::Limit => Status::Exhausted,
        Stop::NoDecision(ref error) if error.contains("OpenRouter") => Status::Error(error.clone()),
        _ => Status::NotAchieved,
    };
    let answer = outcome
        .page
        .as_ref()
        .map(|page| {
            format!(
                "{} — {}\n{}",
                page.title,
                page.url,
                page.text.chars().take(600).collect::<String>()
            )
        })
        .unwrap_or_default();
    let usage = Usage {
        actions: outcome.steps.len(),
        ..Usage::default()
    };
    RunResult {
        messages: vec![],
        data: None,
        status,
        answer: format!("Stopped: {}.\n{answer}", outcome.stop),
        usage,
        millis: started.elapsed().as_millis() as u64,
    }
}

fn final_page(driver: &dyn Driver, page: &Observation) -> String {
    let body = driver
        .read(0, 4000)
        .map(|text| text.text)
        .unwrap_or_default();
    format!("{}\nFull text:\n{body}", describe_page(page))
}

/// An answer without its RESULT line: the decision model reads whether it reports success.
fn stated(decider: &dyn Decider, goal: &str, answer: &str) -> Status {
    let question = Question::Noul {
        instructions: json!("Does the answer report that the goal was reached?"),
    };
    let questions = [("reached".to_string(), question)].into();
    match decider.decide(&json!({"goal": goal, "answer": answer}), &questions) {
        Ok(decision) => match decision.answers.get("reached") {
            Some(Answer::Noul(p)) if *p >= 0.5 => Status::Achieved,
            _ => Status::NotAchieved,
        },
        Err(_) => Status::NotAchieved,
    }
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
