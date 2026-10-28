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

use protocol::{Label, Outcome};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::style;
use super::wrap::{cut, wrap};
use crate::tui::printable::printable;

/// Rows of output a finished call keeps: the first and the last, with how many were left out.
const HEAD: usize = 2;
const TAIL: usize = 3;

/// Where a call is.
pub(crate) enum State<'a> {
    Running,
    Done(&'a Outcome),
}

/// The block for a call of `name` that reads as `label`.
pub(crate) fn lines(name: &str, label: &Label, state: &State, width: usize) -> Vec<Line<'static>> {
    let mut out = head(name, label, state, width);
    if let State::Done(outcome) = state {
        out.extend(body(name, outcome, width));
    }
    out
}

/// A call the person said no to: not a failure, so not red; what they said is under it.
pub(crate) fn declined(label: &Label, said: &str, width: usize) -> Vec<Line<'static>> {
    let mut out = head("", label, &State::Running, width);
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
pub(crate) fn head(name: &str, label: &Label, state: &State, width: usize) -> Vec<Line<'static>> {
    let (verb, bullet) = match state {
        State::Running => (&label.doing, style::dim()),
        State::Done(outcome) if failed(name, outcome) => (&label.done, style::failed()),
        State::Done(_) => (&label.done, style::done()),
    };
    let head = Line::from(vec![
        Span::styled(verb.clone(), style::bold()),
        Span::raw(" "),
        Span::raw(printable(&label.subject)),
    ]);
    wrap(
        &head,
        width,
        &Line::from(Span::styled("• ", bullet)),
        &Line::from(Span::styled("  │ ", style::dim())),
    )
}

/// What a finished call printed or why it failed, under its head.
pub(crate) fn body(name: &str, outcome: &Outcome, width: usize) -> Vec<Line<'static>> {
    let rows = match outcome {
        Outcome::Failed { why, .. } | Outcome::Fatal { why } => {
            vec![(printable(why), style::failed())]
        }
        Outcome::Done { report } => output_rows(name, report),
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

fn failed(name: &str, outcome: &Outcome) -> bool {
    match outcome {
        Outcome::Failed { .. } | Outcome::Fatal { .. } => true,
        Outcome::Done { report } => name == "bash" && !exit_zero(report),
    }
}

/// What a call running does: `Running cargo test`.
pub(crate) fn doing(label: &Label) -> String {
    format!("{} {}", label.doing, label.subject)
        .trim()
        .to_string()
}

/// The rows worth showing under a finished call. A command shows what it printed, and its exit
/// when that was not 0; the rest show their report's first row.
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
