//! What a run reports: the live trace on stderr, the answer (or PASS/FAIL, or the data) on stdout,
//! `--json` (schema 3) and the `--trace` file.

use std::sync::Mutex;
use std::time::Instant;

use agent::agent::Spend as ChatSpend;
use contracts::view::Observation;
use decision_loop::expect::Verdict;
use decision_loop::meter::{Spend, thousands};
use decision_loop::step::GoalStep;
use serde_json::{Value, json};

use crate::exit;
use crate::run::{Options, RunResult, Status};

/// The `--json` schema version: a public contract (see CHANGELOG.md).
const SCHEMA: u32 = 3;

/// Live trace on stderr, and every event kept for `--trace`.
pub(crate) struct Trace {
    pub(crate) started: Instant,
    pub(crate) events: Mutex<Vec<Value>>,
    pub(crate) quiet: bool,
}

impl Trace {
    fn line(&self, text: &str, mut event: Value) {
        let at = self.started.elapsed().as_millis() as u64;
        if !self.quiet {
            eprintln!("{:>6.1}s {text}", at as f64 / 1000.0);
        }
        event["ms"] = json!(at);
        self.events.lock().unwrap().push(event);
    }

    pub(crate) fn call(&self, name: &str, args: &Value) {
        self.line(&call_line(name, args), json!({"call": name, "args": args}));
    }

    pub(crate) fn step(&self, step: &GoalStep) {
        self.line(
            &format!("    {}", step.trace_line()),
            json!({"step": step.trace_line()}),
        );
    }
}

/// A tool call of the agent as the trace shows it: `▸ browser goal: … · values: [...]`, with only
/// the arguments it was given.
fn call_line(name: &str, args: &Value) -> String {
    let parts: Vec<String> = args
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, value)| !matches!(value, Value::Null | Value::Bool(false)))
        .filter(|(_, value)| value.as_str().is_none_or(|text| !text.is_empty()))
        .filter(|(_, value)| value.as_array().is_none_or(|list| !list.is_empty()))
        .map(|(key, value)| match value.as_str() {
            Some(text) => format!("{key}: {}", one_line(text, 160)),
            None => format!("{key}: {}", one_line(&value.to_string(), 160)),
        })
        .collect();
    format!("▸ {name} {}", parts.join(" · "))
}

/// `text` on one line, at most `limit` characters.
fn one_line(text: &str, limit: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.char_indices().nth(limit) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line,
    }
}

pub(crate) fn status_name(status: &Status) -> &'static str {
    match status {
        Status::Achieved => "achieved",
        Status::NotAchieved => "not_achieved",
        Status::Unstated => "unstated",
        Status::Exhausted => "exhausted",
        Status::Error(_) => "error",
    }
}

/// Time not spent waiting for a model: the browser loading, acting and settling.
fn browser_ms(result: &RunResult) -> u64 {
    result
        .millis
        .saturating_sub(result.chat.millis + result.decision.millis)
}

fn cost(result: &RunResult) -> f64 {
    result.chat.cost + result.decision.cost
}

fn chat_json(spend: &ChatSpend) -> Value {
    json!({
        "model": spend.model,
        "calls": spend.calls,
        "input_tokens": spend.input_tokens,
        "cached_tokens": spend.cached_tokens,
        "output_tokens": spend.output_tokens,
        "cost_usd": spend.cost,
        "ms": spend.millis,
    })
}

/// The closing breakdown: where the time went, and each model's calls, tokens and money.
fn spending(result: &RunResult, options: &Options) -> String {
    let seconds = |ms: u64| ms as f64 / 1000.0;
    let (chat, decision) = (&result.chat, &result.decision);
    let mut out = format!(
        "  time     {:.1}s agent · {:.1}s decision model · {:.1}s browser\n",
        seconds(chat.millis),
        seconds(decision.millis),
        seconds(browser_ms(result)),
    );
    let named = |model: &str, asked: &str| match model.is_empty() {
        true => asked.to_string(),
        false => model.to_string(),
    };
    if chat.calls > 0 {
        let cached = match chat.cached_tokens {
            0 => String::new(),
            cached => format!(" ({} cached)", thousands(cached)),
        };
        out.push_str(&format!(
            "  agent    {} · {} calls · {} in{cached} · {} out · ${:.4}\n",
            named(&chat.model, &options.chat_model),
            chat.calls,
            thousands(chat.input_tokens),
            thousands(chat.output_tokens),
            chat.cost,
        ));
    }
    if decision.calls > 0 {
        out.push_str(&decision_line(
            decision,
            &named(&decision.model, &options.decision_model),
        ));
    }
    out
}

fn decision_line(spend: &Spend, model: &str) -> String {
    format!(
        "  decision {model} · {} calls · {} in · {} out · ${:.4}\n",
        spend.calls,
        thousands(spend.input_tokens),
        thousands(spend.output_tokens),
        spend.cost,
    )
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
    let mut summary = json!({
        "schema": SCHEMA,
        "status": status_name(status),
        "exit_code": code,
        "answer": result.answer,
        "error": match status { Status::Error(error) => json!(error), _ => Value::Null },
        "expects": verdicts.iter().map(|v| json!({"text": v.text, "passed": v.passed, "probability": v.probability})).collect::<Vec<_>>(),
        "screenshot": result.screenshot,
        "final_url": page.map(|page| page.location.clone()),
        "duration_ms": result.millis,
        "usage": {
            "agent": chat_json(&result.chat),
            "decision": result.decision.json(),
            "browser_ms": browser_ms(result),
            "actions": result.actions,
            "cost_usd": cost(result),
        },
    });
    match &result.data {
        Some(Ok(value)) => summary["data"] = value.clone(),
        Some(Err(error)) => summary["data_error"] = json!(error),
        None => {}
    }
    if let Some(path) = &options.trace {
        let mut full = summary.clone();
        full["goal"] = json!(options.goal);
        full["events"] = json!(*trace.events.lock().unwrap());
        let text = serde_json::to_string_pretty(&full).unwrap_or_default();
        if let Err(error) = std::fs::write(path, text) {
            eprintln!("lxsurf: could not write the trace: {error}");
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
        cost(result),
    );
    eprint!("{}", spending(result, options));
    if let Some(path) = &result.screenshot {
        eprintln!("  picture  {}", path.display());
    }
    if let Status::Error(error) = status {
        eprintln!("lxsurf: {error}");
    }
    if options.check {
        let line = result.answer.lines().next().unwrap_or("");
        println!(
            "{}",
            if code == exit::OK {
                "PASS".into()
            } else {
                format!("FAIL: {line}")
            }
        );
    } else if let Some(data) = &result.data {
        // A pipeline reads stdout: the JSON alone, on one line; the prose answer goes to stderr.
        match data {
            Ok(value) => println!("{value}"),
            Err(error) => eprintln!("lxsurf: {error}"),
        }
        if !result.answer.is_empty() {
            eprintln!("{}", result.answer);
        }
    } else if !result.answer.is_empty() {
        println!("{}", result.answer);
    }
}

/// A view as a person reads it in the terminal: where it is, then its outline, or each control
/// with its ref when the view has no outline.
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

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
