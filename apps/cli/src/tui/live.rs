//! The live area under the history: the call running, with the hands' latest actions under it;
//! how long the task has worked and how to stop it; what is queued; then the question open, or
//! the composer with the commands `/` offers and the mode.
//!
//! ```text
//! ⠹ Running cargo test --workspace
//!
//! ⠹ Working (12s · esc to interrupt)
//!   ↳ then update the changelog
//!
//! ❯ Type a task, / for commands
//!   plan mode · shift+tab to leave
//! ```

use std::time::Instant;

use ratatui::text::{Line, Span};

use super::app::App;
use super::cells::duration;
use super::screen::Cursor;
use super::style;
use super::tool_cell::{self, State};
use super::wrap::cut;
use crate::printable::printable;

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const FRAME_MS: u128 = 80;
const PLACEHOLDER: &str = "Type a task, / for commands";

/// The live rows at `now`, and where the cursor goes among them.
pub fn render(app: &App, now: Instant) -> (Vec<Line<'static>>, Option<Cursor>) {
    let width = app.width.max(10);
    let spinner = |since: Instant| {
        let frame = now.duration_since(since).as_millis() / FRAME_MS;
        Span::styled(
            SPINNER[frame as usize % SPINNER.len()].to_string(),
            style::code(),
        )
    };
    let mut out: Vec<Line<'static>> = vec![];
    // While a question is open it says what the call would do; the call shows again after.
    if let Some((name, args, started)) = app.running.as_ref().filter(|_| app.select.is_none()) {
        let mut rows = tool_cell::lines(name, args, State::Running, width);
        if let Some(first) = rows.first_mut() {
            first.spans[0] = spinner(*started);
            first.spans.insert(1, Span::raw(" "));
        }
        out.extend(rows);
        for step in &app.steps {
            let step = cut(&printable(step), width.saturating_sub(4));
            out.push(Line::from(Span::styled(
                format!("  └ {step}"),
                style::dim(),
            )));
        }
        out.push(Line::default());
    }
    if let Some(started) = app.busy.filter(|_| app.select.is_none()) {
        let took = duration(now.duration_since(started).as_millis() as u64);
        let state = if app.stopping {
            "stopping"
        } else {
            "esc to interrupt"
        };
        out.push(Line::from(vec![
            spinner(started),
            Span::raw(" "),
            Span::styled("Working", style::bold()),
            Span::styled(
                format!(" ({} · {state})", whole_seconds(&took)),
                style::dim(),
            ),
        ]));
    }
    for entry in &app.queued {
        let entry = cut(
            &printable(&entry.replace('\n', " ")),
            width.saturating_sub(4),
        );
        out.push(Line::from(Span::styled(
            format!("  ↳ {entry}"),
            style::dim(),
        )));
    }
    if !out.is_empty() && out.last() != Some(&Line::default()) {
        out.push(Line::default());
    }
    if let Some((select, _)) = &app.select {
        let (rows, cursor) = select.render(width);
        let top = out.len();
        out.extend(rows);
        let cursor = cursor.map(|(row, column)| Cursor {
            row: top + row,
            column,
        });
        return (out, cursor);
    }
    let prompt = Span::styled("❯ ", style::bold());
    let (rows, (row, column)) = app.composer.render(prompt, PLACEHOLDER, width);
    let cursor = Cursor {
        row: out.len() + row,
        column,
    };
    out.extend(rows);
    let offers = app.offers();
    for (at, (name, what)) in offers.iter().enumerate() {
        let marked = at == app.offered.min(offers.len() - 1);
        let name_style = if marked { style::bold() } else { style::dim() };
        // Cut, not wrapped: the names stay in a column.
        let what = cut(&printable(what), width.saturating_sub(14));
        out.push(Line::from(vec![
            Span::styled(format!("  /{name:<10}"), name_style),
            Span::styled(format!(" {what}"), style::dim()),
        ]));
    }
    if let Some(footer) = footer(app) {
        out.push(Line::from(Span::styled(
            format!("  {footer}"),
            style::dim(),
        )));
    }
    (out, Some(cursor))
}

fn footer(app: &App) -> Option<String> {
    if app.quitting.is_some() {
        return Some("ctrl-c again to quit".into());
    }
    match app.mode.as_str() {
        "default" => None,
        "plan" => Some("plan mode · shift+tab to leave".into()),
        mode => Some(format!("{mode} mode · shift+tab for plan")),
    }
}

/// `12s` rather than `2.1s`: the status counts up once a second.
fn whole_seconds(took: &str) -> String {
    match took.strip_suffix('s').and_then(|s| s.parse::<f64>().ok()) {
        Some(seconds) => format!("{}s", seconds as u64),
        None => took.to_string(),
    }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
