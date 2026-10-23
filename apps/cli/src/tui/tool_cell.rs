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
#[path = "edit_diff.rs"]
mod edit_diff;
use super::wrap::{cut, wrap};
use crate::printable::printable;

/// Rows of output a finished call keeps: the first and the last, with how many were left out.
const HEAD: usize = 2;
const TAIL: usize = 3;

/// Where a call is.
pub(crate) enum State<'a> {
    Running,
    Done(&'a Result<ToolOutput, ToolError>),
}

/// The block for a call of `name` with `args`.
pub(crate) fn lines(name: &str, args: &Value, state: State, width: usize) -> Vec<Line<'static>> {
    let mut out = head(name, args, &state, width);
    if let State::Done(outcome) = state {
        out.extend(body(name, outcome, width));
    }
    out
}

/// A call the person said no to: not a failure, so not red; what they said is under it.
pub(crate) fn declined(name: &str, args: &Value, said: &str, width: usize) -> Vec<Line<'static>> {
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
pub(crate) fn head(name: &str, args: &Value, state: &State, width: usize) -> Vec<Line<'static>> {
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
pub(crate) fn body(
    name: &str,
    outcome: &Result<ToolOutput, ToolError>,
    width: usize,
) -> Vec<Line<'static>> {
    if name == "edit"
        && let Ok(output) = outcome
        && let Some(diff) = output
            .report
            .split_once("\n\nDiff:\n")
            .map(|(_, diff)| diff)
    {
        let mut out = body(
            "edit",
            &Ok(output
                .report
                .lines()
                .next()
                .unwrap_or_default()
                .to_string()
                .into()),
            width,
        );
        out.extend(edit_diff::render(diff, width));
        return out;
    }
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

/// What a call did, in a few words: `Ran cargo test`.
pub(crate) fn summary(name: &str, args: &Value) -> String {
    let what = describe(name, args);
    format!("{} {}", what.verb.1, what.subject)
        .trim()
        .to_string()
}

/// What a call running does: `Running cargo test`.
pub(crate) fn doing(name: &str, args: &Value) -> String {
    let what = describe(name, args);
    format!("{} {}", what.verb.0, what.subject)
        .trim()
        .to_string()
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
        "bash" => {
            let mut subject = flat(text("command"));
            if !text("workdir").is_empty() {
                subject = format!("{subject} (in {})", text("workdir"));
            }
            what(("Running", "Ran"), subject)
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
            what(
                ("Asking", "Asked"),
                super::cells::count(n as u64, "question"),
            )
        }
        "agents" => {
            let (verb, subject) = crate::agents::spec::describe(args);
            what(verb, flat(&subject))
        }
        "jobs" => {
            let (verb, subject) = crate::jobs::spec::describe(args);
            what(verb, flat(&subject))
        }
        "window" => window(args),
        _ => what(("Calling", "Called"), name.to_string()),
    }
}

/// A call on the window: what it opens, which tab it acts in, and how.
fn window(args: &Value) -> What {
    let text = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let what = |verb, subject: String| What { verb, subject };
    let tab = text(args, "tab");
    match text(args, "action").as_str() {
        "open" => {
            let one = [args.clone()];
            let tabs = args
                .get("tabs")
                .and_then(Value::as_array)
                .map_or(&one[..], Vec::as_slice);
            let named: Vec<String> = (tabs.iter())
                .map(|tab| match text(tab, "url").as_str() {
                    "" => text(tab, "command"),
                    url => url.to_string(),
                })
                .filter(|name| !name.is_empty())
                .collect();
            what(("Opening", "Opened"), named.join(", "))
        }
        "interact" if !text(args, "goal").is_empty() => what(
            ("Working in", "Worked in"),
            format!("{tab}: {}", text(args, "goal")),
        ),
        "interact" => {
            let keys = (args
                .get("keys")
                .and_then(Value::as_array)
                .into_iter()
                .flatten())
            .map(|key| format!("[{}]", key.as_str().unwrap_or("?")));
            let mut typed: Vec<String> = vec![text(args, "input")];
            typed.extend(keys);
            let typed = typed.join(" ").trim().to_string();
            match typed.is_empty() {
                true => what(("Reading", "Read"), tab),
                false => what(("Typing in", "Typed in"), format!("{tab}: {typed}")),
            }
        }
        "show" => what(("Showing", "Showed"), format!("tab {tab}")),
        "screenshot" => what(("Picturing", "Pictured"), format!("tab {tab}")),
        "close" => {
            let tabs: Vec<&str> = (args
                .get("tabs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten())
            .filter_map(Value::as_str)
            .collect();
            match tabs.is_empty() {
                true => what(("Closing", "Closed"), format!("tab {tab}")),
                false => what(("Closing", "Closed"), format!("tabs {}", tabs.join(", "))),
            }
        }
        "resize" => {
            let size = |key: &str| args.get(key).and_then(Value::as_u64).unwrap_or(0);
            what(
                ("Sizing", "Sized"),
                format!("the window to {}×{}", size("width"), size("height")),
            )
        }
        _ => what(("Listing", "Listed"), "the window's tabs".into()),
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
            .and_then(|line| line.rfind("(").and_then(|at| line.get(at..)))
            .map(|counts| vec![(counts.trim_end_matches('.').to_string(), style::dim())])
            .unwrap_or_default(),
        "read" => vec![],
        // What the person answered, without the report's own heading.
        "ask" => rows(report).into_iter().skip(1).collect(),
        // The agent's answer, without the line that names it.
        // What agents said, without the tags that frame it for the model.
        "agents" => rows(report)
            .into_iter()
            .filter(|(row, _)| !row.starts_with("<agent_result") && row != "</agent_result>")
            .collect(),
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
