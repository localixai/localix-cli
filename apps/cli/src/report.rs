//! What a run reports: the live trace on stderr, the answer or PASS/FAIL on stdout, `--json`
//! (schema 2) and the `--trace` file.

use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Value, json};

use agent::tool::Observation;
use decision_loop::expect::Verdict;
use decision_loop::meter::thousands;
use decision_loop::step::GoalStep;

use crate::exit;
use crate::run::{Options, RunResult, Status};

/// Live trace on stderr, and every event kept for `--trace`.
pub(crate) struct Trace {
    pub(crate) started: Instant,
    pub(crate) events: Mutex<Vec<Value>>,
    pub(crate) quiet: bool,
}

impl Trace {
    fn line(&self, text: &str, event: Value) {
        let at = self.started.elapsed().as_millis() as u64;
        if !self.quiet {
            eprintln!("{:>6.1}s {text}", at as f64 / 1000.0);
        }
        let mut event = event;
        event["ms"] = json!(at);
        self.events.lock().unwrap().push(event);
    }

    pub(crate) fn step(&self, step: &GoalStep) {
        self.line(
            &format!("    {}", step.trace_line()),
            json!({"step": step.trace_line()}),
        );
    }
}

pub(crate) fn status_name(status: &Status) -> &'static str {
    match status {
        Status::Achieved => "achieved",
        Status::NotAchieved => "not_achieved",
        Status::Exhausted => "exhausted",
        Status::Error(_) => "error",
    }
}

/// Time not spent waiting for the model: the browser loading, acting and settling.
fn browser_ms(result: &RunResult) -> u64 {
    result.millis.saturating_sub(result.decision.millis)
}

/// The closing breakdown: where the time went, and the model's calls, tokens and money.
fn spending(result: &RunResult, options: &Options) -> String {
    let spend = &result.decision;
    let seconds = |ms: u64| ms as f64 / 1000.0;
    let mut out = format!(
        "  time     {:.1}s decision model · {:.1}s browser\n",
        seconds(spend.millis),
        seconds(browser_ms(result)),
    );
    if spend.calls > 0 {
        let model = if spend.model.is_empty() {
            &options.decision_model
        } else {
            &spend.model
        };
        out.push_str(&format!(
            "  model    {model} · {} calls · {} in · {} out · ${:.4}\n",
            spend.calls,
            thousands(spend.input_tokens),
            thousands(spend.output_tokens),
            spend.cost,
        ));
    }
    out
}

pub(crate) fn report(
    options: &Options,
    result: &RunResult,
    verdicts: &[Verdict],
    page: Option<&Observation>,
    code: i32,
    trace: &Trace,
) {
    let status = &result.status;
    let summary = json!({
        "schema": 2,
        "status": status_name(status),
        "exit_code": code,
        "answer": result.answer,
        "error": match status { Status::Error(error) => json!(error), _ => Value::Null },
        "expects": verdicts.iter().map(|v| json!({"text": v.text, "passed": v.passed, "probability": v.probability})).collect::<Vec<_>>(),
        "tool": result.tool,
        "screenshot": result.screenshot,
        "final_location": page.map(|page| page.location.clone()),
        "duration_ms": result.millis,
        "usage": {
            "decision": result.decision.json(),
            "browser_ms": browser_ms(result),
            "actions": result.actions,
            "cost_usd": result.decision.cost,
        },
    });
    if let Some(path) = &options.trace {
        let mut full = summary.clone();
        full["goal"] = json!(options.goal);
        full["events"] = json!(*trace.events.lock().unwrap());
        if let Err(error) = std::fs::write(
            path,
            serde_json::to_string_pretty(&full).unwrap_or_default(),
        ) {
            eprintln!("decision-loop: could not write the trace: {error}");
        }
    }
    if options.json {
        println!("{summary}");
        return;
    }
    for verdict in verdicts {
        let mark = if verdict.passed { "✓" } else { "✗" };
        let probability = verdict
            .probability
            .map(|p| format!(" ({:.0}%)", p * 100.0))
            .unwrap_or_default();
        eprintln!("{mark} expect: {}{probability}", verdict.text);
    }
    eprintln!(
        "{} in {:.1}s · {} actions · ${:.4}",
        status_name(status),
        result.millis as f64 / 1000.0,
        result.actions,
        result.decision.cost,
    );
    eprint!("{}", spending(result, options));
    if let Some(path) = &result.screenshot {
        eprintln!("  picture  {}", path.display());
    }
    if let Status::Error(error) = status {
        eprintln!("decision-loop: {error}");
    }
    if options.check {
        println!(
            "{}",
            if code == exit::OK {
                "PASS".to_string()
            } else {
                format!("FAIL: {}", result.answer.lines().next().unwrap_or(""))
            }
        );
    } else if !result.answer.is_empty() {
        println!("{}", result.answer);
    }
}

/// A view as a person reads it in the terminal: where it is, then its outline, or each control
/// with its ref when the tool has no outline.
pub(crate) fn view_table(view: &Observation) -> String {
    let mut out = format!("{} — {}\n", view.title, view.location);
    if !view.outline.is_empty() {
        out.push_str(&view.outline);
        out.push('\n');
        return out;
    }
    for element in &view.elements {
        let scope = if element.dialog { " (dialog)" } else { "" };
        out.push_str(&format!(
            "[{}] {}{scope}\n",
            element.reference,
            element.describe()
        ));
    }
    if view.omitted > 0 {
        out.push_str(&format!("… {} more controls past the cap\n", view.omitted));
    }
    out
}
