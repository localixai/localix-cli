//! The blocks of a session's history, other than tool calls ([`super::tool_cell`]): what the
//! person asked, what the model answered, how a task ended, and what went wrong. Each is a pure
//! function of its content and the width; the screen puts one blank row between blocks.
//!
//! ```text
//! ❯ make a landing page for a phone shop
//!
//! • The page is in site/index.html.
//!
//! ─ Worked for 48s · 6 turns · $0.0087 ──────────────────────
//! ```

use ratatui::text::{Line, Span};

use super::markdown;
use super::style;
use super::wrap::{line_width, wrap};
use crate::printable::printable;

/// What the person typed.
pub(crate) fn user(text: &str, width: usize) -> Vec<Line<'static>> {
    // `/init` sends a long instruction; the person typed the command.
    let text = match text == crate::commands::extras::INIT {
        true => "/init",
        false => text,
    };
    let first = Line::from(Span::styled("❯ ", style::bold()));
    let rest = Line::from("  ");
    text.trim()
        .lines()
        .enumerate()
        .flat_map(|(at, line)| {
            let line = Line::from(printable(line));
            wrap(&line, width, if at == 0 { &first } else { &rest }, &rest)
        })
        .collect()
}

/// What the model wrote, as markdown under a bullet.
pub(crate) fn agent(text: &str, width: usize) -> Vec<Line<'static>> {
    let rows = markdown::render(text.trim(), width.saturating_sub(2));
    rows.into_iter()
        .enumerate()
        .map(|(at, mut row)| {
            let prefix = if at == 0 { "• " } else { "  " };
            if line_width(&row) > 0 || at == 0 {
                row.spans.insert(0, Span::raw(prefix));
            }
            row
        })
        .collect()
}

/// How a task ended.
pub(crate) struct End<'a> {
    /// As the run reports it: `achieved`, `not_achieved`, `unstated`, `exhausted`, `error`,
    /// `interrupted`.
    pub status: &'a str,
    pub millis: u64,
    pub turns: u64,
    pub cost: f64,
}

/// A rule across the width with how long the task took and what it cost.
pub(crate) fn end(end: &End, width: usize) -> Vec<Line<'static>> {
    let verb = match end.status {
        "interrupted" => "Stopped after",
        "error" => "Failed after",
        // A limit of turns or of time: the error line under it says which.
        "exhausted" => "Ran out after",
        "not_achieved" => "Not achieved after",
        _ => "Worked for",
    };
    let turns = count(end.turns, "turn");
    let text = format!(
        "─ {verb} {} · {turns} · ${:.4} ",
        duration(end.millis),
        end.cost
    );
    let fill = width.saturating_sub(super::wrap::width(&text));
    vec![Line::from(Span::styled(
        format!("{text}{}", "─".repeat(fill)),
        style::dim(),
    ))]
}

/// Tasks shown at most of a resumed session; the earlier ones are counted.
const EARLIER: usize = 20;

/// A resumed session's tasks as blocks: how many there were, then each task, its answer, and how
/// it ended when it did not end well. Only the tasks and answers are kept, not their calls.
pub(crate) fn earlier(turns: &[contracts::history::Turn], width: usize) -> Vec<Vec<Line<'static>>> {
    let skipped = turns.len().saturating_sub(EARLIER);
    let mut said = count(turns.len() as u64, "earlier task");
    if skipped > 0 {
        said.push_str(&format!(" · the last {EARLIER} shown"));
    }
    let mut out = vec![note(&said, width)];
    for turn in &turns[skipped..] {
        out.push(user(&super::steer::shown(&turn.goal), width));
        if !turn.answer.trim().is_empty() {
            out.push(agent(&turn.answer, width));
        }
        if !matches!(turn.status.as_str(), "achieved" | "unstated") {
            let (style, mark, word, _) = super::tree::ending(&turn.status);
            out.push(vec![Line::from(vec![
                Span::styled(format!("{mark} "), style),
                Span::styled(word, style::dim()),
            ])]);
        }
    }
    out
}

/// Something that went wrong outside a tool call: the model, the network, the session.
pub(crate) fn error(text: &str, width: usize) -> Vec<Line<'static>> {
    let line = Line::from(Span::styled(printable(text.trim()), style::failed()));
    wrap(
        &line,
        width,
        &Line::from(Span::styled("■ ", style::failed())),
        &Line::from("  "),
    )
}

/// A quiet line from llx itself: a setting changed, a session resumed.
/// Its rows are kept as they are (a table, a list of commands), and only a row too wide wraps.
pub(crate) fn note(text: &str, width: usize) -> Vec<Line<'static>> {
    let mut out = vec![];
    for row in text.trim_matches('\n').lines() {
        let row = printable(row.trim_end());
        match super::wrap::width(&row) + 2 <= width {
            true => out.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(row, style::dim()),
            ])),
            false => {
                // A table's row (a label, two spaces or more, then its text) wraps under its
                // second column, so the columns stay; any other row hangs by two.
                let trimmed = row.trim_start();
                let label = (trimmed.find("  ").and_then(|at| trimmed.get(at..)))
                    .map(|rest| row.len() - rest.trim_start().len())
                    .filter(|&end| {
                        (row.get(..end)).is_some_and(|head| super::wrap::width(head) * 2 < width)
                    })
                    .unwrap_or(0);
                let (label, body) = row.split_at(label);
                let under = match label.is_empty() {
                    true => "    ".to_string(),
                    false => " ".repeat(2 + super::wrap::width(label)),
                };
                let first = Line::from(Span::styled(format!("  {label}"), style::dim()));
                let body = Line::from(Span::styled(body.to_string(), style::dim()));
                out.extend(wrap(&body, width, &first, &Line::from(under)));
            }
        }
    }
    out
}

/// The session's first row: the version, then what it runs with.
/// It stays one row: the last fact (the folder) loses its start when the row is too narrow.
pub(crate) fn header(version: &str, facts: &[&str], width: usize) -> Vec<Line<'static>> {
    let mut line = Line::from(vec![Span::styled(format!("llx {version}"), style::bold())]);
    let facts: Vec<String> = facts
        .iter()
        .filter(|f| !f.is_empty())
        .map(|f| printable(f))
        .collect();
    for (at, fact) in facts.iter().enumerate() {
        let mut fact = fact.clone();
        if at + 1 == facts.len() {
            let room = width.saturating_sub(line_width(&line) + 3);
            fact = cut_start(&fact, room);
        }
        line.spans
            .push(Span::styled(format!(" · {fact}"), style::dim()));
    }
    vec![line]
}

/// `text` without its start when wider than `columns`: `…/scratchpad/ws1`.
fn cut_start(text: &str, columns: usize) -> String {
    if super::wrap::width(text) <= columns {
        return text.to_string();
    }
    let reversed: String = text.chars().rev().collect();
    let kept: String = super::wrap::cut(&reversed, columns).chars().rev().collect();
    kept
}

/// `48s`, `1m 05s`, `1h 02m`.
pub(crate) fn duration(millis: u64) -> String {
    match millis {
        0..10_000 => format!("{:.1}s", millis as f64 / 1000.0),
        _ => clock(millis / 1000),
    }
}

/// `n` of `noun`: `1 turn`, `3 turns`.
pub(crate) fn count(n: u64, noun: &str) -> String {
    match n {
        1 => format!("1 {noun}"),
        n => format!("{n} {noun}s"),
    }
}

/// Whole `seconds` as they count up: `12s`, `3m 08s`, `1h 02m`.
pub(crate) fn clock(seconds: u64) -> String {
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h {:02}m", seconds / 3600, seconds / 60 % 60),
    }
}

#[cfg(test)]
#[path = "cells_tests.rs"]
mod tests;
