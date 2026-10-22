//! A tool call as a block: a bullet, what it does in a few words, and the end of what it printed.
//!
//! ```text
//! • Ran cargo test --workspace
//!   │ --all-targets
//!   └ test result: ok. 133 passed
//!     … +41 lines
//!     finished in 0.9s
//! ```
//!
//! The bullet is dim while the call runs (the live area draws a spinner there), green when it is
//! done and red when it failed. Everything a model or a tool wrote is escaped first, so it cannot
//! redraw the terminal.

use contracts::tool::{ToolError, ToolOutput};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::style;
use super::wrap::{cut, wrap};
use crate::printable::printable;

/// Rows of output a finished call keeps: the first and the last, with how many were left out.
const HEAD: usize = 2;
const TAIL: usize = 3;

/// Where a call is.
pub enum State<'a> {
    Running,
    Done(&'a Result<ToolOutput, ToolError>),
}

/// The block for a call of `name` with `args`.
pub fn lines(name: &str, args: &Value, state: State, width: usize) -> Vec<Line<'static>> {
    let mut out = head(name, args, &state, width);
    if let State::Done(outcome) = state {
        out.extend(body(name, outcome, width));
    }
    out
}

/// A call the person said no to: not a failure, so not red; what they said is under it.
pub fn declined(name: &str, args: &Value, said: &str, width: usize) -> Vec<Line<'static>> {
    let mut out = head(name, args, &State::Running, width);
    out[0].spans[0] = Span::styled("• ", style::waiting());
    out[0].spans[1] = Span::styled("Skipped", style::bold());
    let text = match said.trim() {
        "" => "You said no".to_string(),
        said => format!("You said no: {}", printable(said)),
    };
    let line = Line::from(Span::styled(text, style::waiting()));
    out.extend(wrap(
        &line,
        width,
        &Line::from(Span::styled("  └ ", style::dim())),
        &Line::from("    "),
    ));
    out
}

/// The bullet and what the call does, wrapped under a rule.
pub fn head(name: &str, args: &Value, state: &State, width: usize) -> Vec<Line<'static>> {
    let what = describe(name, args);
    let (verb, bullet) = match state {
        State::Running => (what.verb.0, style::dim()),
        State::Done(outcome) if failed(name, outcome) => (what.verb.1, style::failed()),
        State::Done(_) => (what.verb.1, style::done()),
    };
    let head = Line::from(vec![
        Span::styled(verb.to_string(), style::bold()),
        Span::raw(" "),
        Span::raw(printable(&what.subject)),
    ]);
    wrap(
        &head,
        width,
        &Line::from(Span::styled("• ", bullet)),
        &Line::from(Span::styled("  │ ", style::dim())),
    )
}

/// What a finished call printed or why it failed, under its head.
pub fn body(
    name: &str,
    outcome: &Result<ToolOutput, ToolError>,
    width: usize,
) -> Vec<Line<'static>> {
    let rows = match outcome {
        Err(error) => vec![(printable(&error.to_string()), style::failed())],
        Ok(output) => output_rows(name, &output.report),
    };
    let mut out = vec![];
    for (at, (text, style)) in shorten(rows).into_iter().enumerate() {
        let prefix = if at == 0 { "  └ " } else { "    " };
        // An output row is cut, not wrapped: a long log line is noise past the first row.
        let text = cut(&text, width.saturating_sub(4).max(1));
        out.push(Line::from(vec![
            Span::styled(prefix, style::dim()),
            Span::styled(text, style),
        ]));
    }
    out
}

fn failed(name: &str, outcome: &Result<ToolOutput, ToolError>) -> bool {
    match outcome {
        Err(_) => true,
        Ok(output) => name == "bash" && !exit_zero(&output.report),
    }
}

struct What {
    /// The verb while running and once done: `Running`/`Ran`.
    verb: (&'static str, &'static str),
    subject: String,
}

fn describe(name: &str, args: &Value) -> What {
    let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("").trim();
    let flat = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let what = |verb, subject: String| What { verb, subject };
    match name {
        "bash" if args.get("session").and_then(Value::as_u64).is_some() => session(args),
        "bash" => {
            let mut subject = flat(text("command"));
            if !text("workdir").is_empty() {
                subject = format!("{subject} (in {})", text("workdir"));
            }
            match args.get("tty") == Some(&Value::Bool(true)) {
                true => what(("Starting", "Started"), subject),
                false => what(("Running", "Ran"), subject),
            }
        }
        "read" => {
            let subject = match args.get("offset").and_then(Value::as_u64) {
                Some(offset) => format!("{} from line {offset}", text("path")),
                None => text("path").to_string(),
            };
            what(("Reading", "Read"), subject)
        }
        "edit" if text("old_string").is_empty() => {
            what(("Creating", "Created"), text("path").to_string())
        }
        "edit" => what(("Editing", "Edited"), text("path").to_string()),
        "ask" => {
            let n = args
                .get("questions")
                .and_then(Value::as_array)
                .map_or(1, Vec::len);
            let subject = if n == 1 {
                "1 question".into()
            } else {
                format!("{n} questions")
            };
            what(("Asking", "Asked"), subject)
        }
        "browser" => {
            let flag = |key: &str| args.get(key) == Some(&Value::Bool(true));
            let mut parts: Vec<String> = vec![];
            match args.get("tab") {
                Some(Value::String(tab)) if tab.trim_matches('"') == "new" => {
                    parts.push("a new tab".into())
                }
                Some(Value::Number(n)) => parts.push(format!("tab {n}")),
                Some(Value::String(n)) if n.parse::<u64>().is_ok() => {
                    parts.push(format!("tab {n}"))
                }
                _ => {}
            }
            parts.extend(
                [text("url"), text("goal")]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .map(str::to_string),
            );
            if !text("script").is_empty() {
                parts.push("a script".into());
            }
            if flag("screenshot") {
                parts.push(match text("selector") {
                    "" => "a screenshot".into(),
                    selector => format!("a screenshot of {selector}"),
                });
            }
            if let Some(closed) = args.get("close_tab").filter(|v| !v.is_null()) {
                let tab = format!("tab {}", closed.to_string().trim_matches('"'));
                if parts.is_empty() {
                    return what(("Closing", "Closed"), tab);
                }
                parts.push(format!("closing {tab}"));
            }
            match (parts.is_empty(), flag("close")) {
                (true, true) => what(("Closing", "Closed"), "the browser".into()),
                (true, false) => what(("Reading", "Read"), "the page".into()),
                (false, _) => what(("Browsing", "Browsed"), flat(&parts.join(" · "))),
            }
        }
        _ => what(("Calling", "Called"), name.to_string()),
    }
}

/// A call on a terminal session: what is typed into it, or that it is read or closed.
fn session(args: &Value) -> What {
    let id = args.get("session").map_or(String::new(), Value::to_string);
    let input = args.get("input").and_then(Value::as_str).unwrap_or("");
    let keys = args
        .get("keys")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    let mut typed = vec![input.trim_end_matches('\n').replace('\n', " \u{23ce} ")];
    typed.extend(keys.map(|key| format!("[{}]", key.as_str().unwrap_or("?"))));
    let typed = typed.join(" ").trim().to_string();
    let what = |verb, subject| What { verb, subject };
    if args.get("close") == Some(&Value::Bool(true)) {
        what(("Closing", "Closed"), format!("session {id}"))
    } else if typed.is_empty() {
        what(("Reading", "Read"), format!("session {id}"))
    } else {
        what(("Typing", "Typed"), format!("into session {id}: {typed}"))
    }
}

/// The rows worth showing under a finished call. A command shows what it printed, and its exit
/// when that was not 0; an edit shows its line counts; the rest show their report's first row.
fn output_rows(name: &str, report: &str) -> Vec<(String, Style)> {
    let rows = |text: &str| -> Vec<(String, Style)> {
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| (printable(line.trim_end()), style::dim()))
            .collect()
    };
    match name {
        "bash" => {
            let (head, streams) = report.split_once('\n').unwrap_or((report, ""));
            let printed: String = streams
                .lines()
                .filter(|line| !matches!(*line, "### stdout" | "### stderr"))
                .collect::<Vec<_>>()
                .join("\n");
            let mut out = rows(&printed);
            if !exit_zero(report) {
                out.push((printable(head), style::failed()));
            } else if out.is_empty() {
                out.push(("(no output)".into(), style::dim()));
            }
            out
        }
        "edit" => report
            .lines()
            .next()
            .and_then(|line| line.rfind("(").map(|at| &line[at..]))
            .map(|counts| vec![(counts.trim_end_matches('.').to_string(), style::dim())])
            .unwrap_or_default(),
        "read" => vec![],
        // What the person answered, without the report's own heading.
        "ask" => rows(report).into_iter().skip(1).collect(),
        _ => rows(report).into_iter().take(1).collect(),
    }
}

/// Whether a command's report says it exited with 0.
fn exit_zero(report: &str) -> bool {
    report.starts_with("exit 0 ")
}

/// At most `HEAD + TAIL + 1` rows: the first, `… +N lines`, the last.
fn shorten(rows: Vec<(String, Style)>) -> Vec<(String, Style)> {
    if rows.len() <= HEAD + TAIL + 1 {
        return rows;
    }
    let hidden = rows.len() - HEAD - TAIL;
    let mut out: Vec<(String, Style)> = rows[..HEAD].to_vec();
    out.push((format!("… +{hidden} lines"), style::dim()));
    out.extend_from_slice(&rows[rows.len() - TAIL..]);
    out
}

#[cfg(test)]
#[path = "tool_cell_tests.rs"]
mod tests;
