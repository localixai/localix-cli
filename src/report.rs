//! What a run reports: the live trace on stderr, the answer or PASS/FAIL on stdout, `--json`
//! (schema 1) and the `--trace` file.

use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Value, json};

use crate::browser::Observation;
use crate::exit;
use crate::expect::Verdict;
use crate::goal_ask::GoalStep;
use crate::meter::thousands;
use crate::orchestrator::{RunResult, Status};
use crate::run::Options;

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

    pub(crate) fn call(&self, name: &str, args: &Value) {
        let mut parts = vec![];
        for key in ["url", "viewport", "goal"] {
            if let Some(value) = args
                .get(key)
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
            {
                parts.push(format!("{key}: {value}"));
            }
        }
        if let Some(values) = args
            .get("values")
            .and_then(Value::as_array)
            .filter(|v| !v.is_empty())
        {
            parts.push(format!("values: {}", Value::Array(values.clone())));
        }
        if let Some(keys) = args
            .get("keys")
            .and_then(Value::as_array)
            .filter(|k| !k.is_empty())
        {
            let shown: Vec<&str> = keys.iter().filter_map(Value::as_str).take(6).collect();
            let more = if keys.len() > 6 { " …" } else { "" };
            parts.push(format!("keys ({}): {}{more}", keys.len(), shown.join(" ")));
        }
        if let Some(script) = args
            .get("script")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            let first: String = script
                .chars()
                .take(60)
                .collect::<String>()
                .replace('\n', " ");
            let more = if script.chars().count() > 60 {
                " …"
            } else {
                ""
            };
            parts.push(format!("script: {first}{more}"));
        }
        if args.get("read").and_then(Value::as_bool) == Some(true) {
            parts.push("read".into());
        }
        self.line(
            &format!("▸ {name} {}", parts.join(" · ")),
            json!({"call": name, "args": args}),
        );
    }

    pub(crate) fn step(&self, step: &GoalStep) {
        self.line(
            &format!("    {}", step.trace_line()),
            json!({"step": step.trace_line()}),
        );
    }
}

fn status_name(status: &Status) -> &'static str {
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
    let usage = &result.usage;
    result
        .millis
        .saturating_sub(usage.planner.millis + usage.decision.millis)
}

/// The closing breakdown: where the time went, and each model's calls, tokens and money.
fn spending(result: &RunResult, options: &Options) -> String {
    let usage = &result.usage;
    let seconds = |ms: u64| ms as f64 / 1000.0;
    let mut out = format!(
        "  time     {:.1}s planner · {:.1}s decision model · {:.1}s browser\n",
        seconds(usage.planner.millis),
        seconds(usage.decision.millis),
        seconds(browser_ms(result)),
    );
    let rows = [
        ("planner", &usage.planner, &options.model),
        ("decision", &usage.decision, &options.decision_model),
    ];
    for (role, spend, asked) in rows {
        if spend.calls == 0 {
            continue;
        }
        let model = if spend.model.is_empty() {
            asked
        } else {
            &spend.model
        };
        let cached = if spend.cached_tokens > 0 {
            format!(" ({} cached)", thousands(spend.cached_tokens))
        } else {
            String::new()
        };
        out.push_str(&format!(
            "  {role:<8} {model} · {} calls · {} in{cached} · {} out · ${:.4}\n",
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
    status: &Status,
    verdicts: &[Verdict],
    page: Option<&Observation>,
    code: i32,
    trace: &Trace,
) {
    let data = result.data.as_ref();
    let usage = &result.usage;
    let mut summary = json!({
        "schema": 1,
        "status": status_name(status),
        "exit_code": code,
        "answer": result.answer,
        "error": match status { Status::Error(error) => json!(error), _ => Value::Null },
        "expects": verdicts.iter().map(|v| json!({"text": v.text, "passed": v.passed, "probability": v.probability, "by": v.by})).collect::<Vec<_>>(),
        "final_url": page.map(|page| page.url.clone()),
        "duration_ms": result.millis,
        "usage": {
            "planner": usage.planner.json(),
            "decision": usage.decision.json(),
            "browser_ms": browser_ms(result),
            "actions": usage.actions,
            "cost_usd": usage.planner.cost + usage.decision.cost,
        },
    });
    match data {
        Some(Ok(value)) => summary["data"] = value.clone(),
        Some(Err(error)) => summary["data_error"] = json!(error),
        None => {}
    }
    if let Some(path) = &options.trace {
        let mut full = summary.clone();
        full["goal"] = json!(options.goal);
        full["events"] = json!(*trace.events.lock().unwrap());
        if let Err(error) = std::fs::write(
            path,
            serde_json::to_string_pretty(&full).unwrap_or_default(),
        ) {
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
        usage.actions,
        usage.planner.cost + usage.decision.cost,
    );
    eprint!("{}", spending(result, options));
    if let Status::Error(error) = status {
        eprintln!("lxsurf: {error}");
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
    } else if let Some(data) = data {
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
