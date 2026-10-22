//! The trace as a person reads it: one short line per call, what went wrong under it, and the
//! closing lines with what the run spent. What the model or a tool wrote is shown with its control
//! characters escaped, so it cannot redraw the terminal.

use contracts::chat::Usage;
use contracts::tool::{ToolError, ToolOutput};
use serde_json::Value;

use super::Summary;
use crate::printable::printable;

/// Characters of an argument or a result line in the trace.
const LINE: usize = 160;
/// Lines of a result `-v` shows.
const RESULT_LINES: usize = 4;

/// A call as one line, by what it does: `▸ bash cargo test`, `▸ read src/lib.rs`, else
/// `▸ browser url: … · goal: …` with only the arguments it was given.
pub fn call(name: &str, args: &Value) -> String {
    let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
    let shown = match name {
        "bash" => {
            let mut line = one_line(text("command"), LINE);
            if !text("workdir").is_empty() {
                line.push_str(&format!("  (in {})", text("workdir")));
            }
            line
        }
        "read" => match args.get("offset").and_then(Value::as_u64) {
            Some(offset) => format!("{} from line {offset}", text("path")),
            None => text("path").to_string(),
        },
        "edit" if text("old_string").is_empty() => format!("{} (new)", text("path")),
        "edit" if args.get("replace_all") == Some(&Value::Bool(true)) => {
            format!("{} (every match)", text("path"))
        }
        "edit" => text("path").to_string(),
        _ => arguments(args),
    };
    format!("▸ {} {}", printable(name), printable(&shown))
}

/// Every argument that says something, as `key: value · key: value`.
fn arguments(args: &Value) -> String {
    let parts: Vec<String> = args
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, value)| !matches!(value, Value::Null | Value::Bool(false)))
        .filter(|(_, value)| value.as_str().is_none_or(|text| !text.is_empty()))
        .filter(|(_, value)| value.as_array().is_none_or(|list| !list.is_empty()))
        .map(|(key, value)| match value.as_str() {
            Some(text) => format!("{key}: {}", one_line(text, LINE)),
            None => format!("{key}: {}", one_line(&value.to_string(), LINE)),
        })
        .collect();
    parts.join(" · ")
}

/// What a result adds to the trace: a failure always; with `-v`, the first lines of a success.
pub fn result(outcome: &Result<ToolOutput, ToolError>, verbose: bool) -> Vec<String> {
    match outcome {
        Err(error) => vec![format!("✗ {}", one_line(&error.to_string(), LINE))],
        Ok(output) if verbose => output
            .report
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(RESULT_LINES)
            .map(|line| format!("  {}", cut(&printable(line), LINE)))
            .collect(),
        Ok(_) => vec![],
    }
}

/// One model turn, for `-v`.
pub fn turn(usage: &Usage) -> String {
    format!(
        "  · {} · {} in · {} out · ${:.4} · {:.1}s",
        usage.model,
        thousands(usage.input_tokens),
        thousands(usage.output_tokens),
        usage.cost,
        usage.millis as f64 / 1000.0
    )
}

/// The closing lines: how it ended, then each model's calls, tokens, money and time.
pub fn summary(summary: &Summary) -> String {
    let (agent, decision) = (summary.agent, summary.decision);
    let seconds = |ms: u64| ms as f64 / 1000.0;
    let turns = match agent.calls {
        1 => "1 turn".to_string(),
        n => format!("{n} turns"),
    };
    let mut out = format!(
        "{} in {:.1}s · {turns} · ${:.4}\n",
        summary.status.replace('_', " "),
        seconds(summary.millis),
        agent.cost + decision.cost
    );
    if agent.calls > 0 {
        let cached = match agent.cached_tokens {
            0 => String::new(),
            cached => format!(" ({} cached)", thousands(cached)),
        };
        out.push_str(&format!(
            "  agent    {} · {} in{cached} · {} out · ${:.4} · {:.1}s\n",
            agent.model,
            thousands(agent.input_tokens),
            thousands(agent.output_tokens),
            agent.cost,
            seconds(agent.millis)
        ));
    }
    if decision.calls > 0 {
        out.push_str(&format!(
            "  hands    {} · {} calls · {} in · {} out · ${:.4} · {:.1}s\n",
            decision.model,
            decision.calls,
            thousands(decision.input_tokens),
            thousands(decision.output_tokens),
            decision.cost,
            seconds(decision.millis)
        ));
    }
    if !summary.session.is_empty() {
        out.push_str(&format!("  session  {}\n", summary.session));
    }
    if let Some(error) = summary.error {
        out.push_str(&format!("lxsurf: {error}\n"));
    }
    out
}

/// What goes to stdout and what to stderr at the end: the answer alone on stdout, or with
/// `--schema` the data alone on stdout (one line, for a pipeline) and the prose on stderr.
pub fn answer(summary: &Summary) -> (String, String) {
    let prose = summary.answer.trim();
    let line = |text: &str| match text.is_empty() {
        true => String::new(),
        false => format!("{text}\n"),
    };
    match summary.data {
        Some(Ok(data)) => (data.to_string(), line(prose)),
        Some(Err(why)) => (String::new(), format!("{}lxsurf: {why}\n", line(prose))),
        None => (prose.to_string(), String::new()),
    }
}

/// `text` on one line, escaped, at most `limit` characters.
fn one_line(text: &str, limit: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    cut(&printable(&line), limit)
}

/// At most `limit` characters of `text`, with `…` when cut.
fn cut(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_string(),
    }
}

/// `12,345`
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
#[path = "human_tests.rs"]
mod tests;
