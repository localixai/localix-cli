//! The sessions under the composer, once the main one has started agents: one
//! row each with what it does now (or how its last task ended), and on the right how
//! long it has worked and the tokens it used. The session shown is bold; while the list has the
//! keys (↓ from an empty prompt), `❯` marks the row chosen.
//!
//! ```text
//!   ● main     Spawning tests · run the suite
//! ❯ ⠹ scout    Reading src/lib.rs                       1m 12s · 54.1k tokens
//!   ✓ tests    done                                         48s · 20.3k tokens
//!   ↓ 2 more
//!   ↑↓ choose · enter view · x stop · esc back
//! ```

use std::time::Instant;

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use serde_json::Value;

use super::agents::{AgentState, SessionView};
use super::app::App;
use super::cells;
use super::style;
use super::tool_cell;
use super::wrap::{cut, width as text_width};
use crate::printable::printable;

/// Agents' rows shown at most; the rest are counted.
pub(super) const ROWS: usize = 5;
/// Columns a name takes at most.
const NAME: usize = 16;
/// A lane whose question waits for the person, open or held until it is shown.
const WAITS: &str = "waiting for you";

/// The list's rows; none while there is one session and no job.
pub fn render(
    app: &App,
    now: Instant,
    spinner: &dyn Fn(Instant) -> Span<'static>,
) -> Vec<Line<'static>> {
    let sessions = &app.views.sessions;
    if sessions.len() == 1 && app.views.jobs.is_empty() {
        return vec![];
    }
    let width = app.width.max(10);
    let order = app.views.order();
    let names = order
        .iter()
        .map(|&at| {
            text_width(sessions[at].title()) + 2 * usize::from(sessions[at].parent.is_some())
        })
        .chain(app.views.jobs.iter().map(|job| text_width(&job.name)))
        .chain([4])
        .max()
        .unwrap_or(4)
        .min(NAME);
    let focus = app.views.focus;
    let shown = order
        .iter()
        .position(|&at| sessions[at].id == app.views.shown);
    let line = |row: usize| {
        let view = &sessions[order[row]];
        let waits = app.waits_for_person(order[row]);
        let mut row = session_row(view, focus == Some(row), shown == Some(row), now, spinner);
        if waits {
            (row.glyph, row.detail) = (Span::styled("?", style::waiting()), WAITS.into());
        }
        self::row(row, names, width)
    };
    let mut out = vec![line(0)];
    // A window of the sessions after the first around the row chosen, else the one shown.
    let rest = order.len() - 1;
    let around = focus
        .or(shown)
        .and_then(|row| row.checked_sub(1))
        .unwrap_or(0);
    let start = around
        .saturating_sub(ROWS - 1)
        .min(rest.saturating_sub(ROWS));
    let end = (start + ROWS).min(rest);
    if start > 0 {
        out.push(more(start, "↑"));
    }
    out.extend((start..end).map(|at| line(at + 1)));
    if end < rest {
        out.push(more(rest - end, "↓"));
    }
    out.extend(jobs(app, order.len(), names, width));
    let on_agent = app.shown().parent.is_some();
    let back = format!("↓ to manage · esc back to {}", app.home_title());
    let hint = match (focus, on_agent) {
        (Some(_), _) => "↑↓ choose · enter view · x stop · esc back",
        (None, true) => &back,
        (None, false) => "↓ to manage · ← sessions",
    };
    out.push(Line::from(Span::styled(
        format!("  {}", cut(hint, width.saturating_sub(2))),
        style::dim(),
    )));
    out
}

/// A session's row: a lane by what its task does or how its last one ended, an agent by its
/// state too (indented under its lane); on the right, how long it worked and its tokens (a
/// lane's count its agents').
fn session_row<'a>(
    view: &'a SessionView,
    chosen: bool,
    shown: bool,
    now: Instant,
    spinner: &dyn Fn(Instant) -> Span<'static>,
) -> Row<'a> {
    let (glyph, detail) = match (&view.parent, &view.state) {
        (None, _) if view.busy.is_some() => (
            view.busy.map_or(Span::raw("●"), spinner),
            doing(&view.running).unwrap_or_else(|| "working".into()),
        ),
        (None, AgentState::Ended(status)) => {
            let (glyph, word) = ended(status);
            let unseen = if view.unseen { " · unseen" } else { "" };
            (glyph, format!("{word}{unseen}"))
        }
        (None, _) => (Span::styled("●", style::code()), "idle".into()),
        (_, AgentState::Idle) => (Span::styled("○", style::dim()), "starting".to_string()),
        (_, AgentState::Working) => (
            view.busy.map_or(Span::raw("○"), spinner),
            doing(&view.running).unwrap_or_else(|| "thinking".into()),
        ),
        (_, AgentState::Ended(status)) => ended(status),
        (_, AgentState::Closed) => (Span::styled("○", style::dim()), "closed".into()),
    };
    Row {
        chosen,
        shown,
        indent: view.parent.is_some(),
        glyph,
        name: view.title(),
        detail,
        right: spent(view, now),
        dimmed: view.state == AgentState::Closed,
    }
}

/// The jobs' rows: a monitor (`≋`, its command, how much is left) or a schedule (`◷`, when).
fn jobs(app: &App, first: usize, names: usize, width: usize) -> Vec<Line<'static>> {
    let mut out = vec![];
    for (at, job) in app.views.jobs.iter().enumerate().take(ROWS) {
        let (glyph, right) = match (job.until, job.next) {
            (Some(until), _) => {
                let left = until.saturating_duration_since(Instant::now()).as_secs();
                let events = cells::count(job.events as u64, "event");
                let right = format!("{events} · {} left", cells::clock(left));
                (Span::styled("≋", style::code()), right)
            }
            (None, next) => {
                let when = next.map(crate::jobs::cron::clock).unwrap_or_default();
                (Span::styled("◷", style::waiting()), format!("next {when}"))
            }
        };
        out.push(row(
            Row {
                chosen: app.views.focus == Some(first + at),
                shown: false,
                indent: false,
                glyph,
                name: &job.name,
                detail: job.detail.clone(),
                right,
                dimmed: false,
            },
            names,
            width,
        ));
    }
    if app.views.jobs.len() > ROWS {
        out.push(more(app.views.jobs.len() - ROWS, "↓"));
    }
    out
}

/// What the calls running do: the first, and how many more.
fn doing(running: &[(String, Value, Instant)]) -> Option<String> {
    let (name, args, _) = running.first()?;
    let first = tool_cell::doing(name, args);
    Some(match running.len() {
        1 => first,
        n => format!("{first} +{} more", n - 1),
    })
}

/// How long the agent has worked and the tokens it used: `1m 08s · 54.1k tokens`.
fn spent(agent: &SessionView, now: Instant) -> String {
    let millis = agent.worked
        + agent
            .busy
            .map_or(0, |since| now.duration_since(since).as_millis() as u64);
    if millis < 1000 && agent.tokens == 0 {
        return String::new();
    }
    format!(
        "{} · {} tokens",
        cells::clock(millis / 1000),
        tokens(agent.tokens)
    )
}

/// `950`, `54.1k`, `1.2M`.
pub(super) fn tokens(count: u64) -> String {
    match count {
        0..1000 => count.to_string(),
        1000..1_000_000 => format!("{:.1}k", count as f64 / 1000.0),
        _ => format!("{:.1}M", count as f64 / 1_000_000.0),
    }
}

struct Row<'a> {
    /// The row the list's keys are on.
    chosen: bool,
    /// An agent's, under its lane.
    indent: bool,
    /// The session on the screen.
    shown: bool,
    glyph: Span<'static>,
    name: &'a str,
    detail: String,
    /// Right-aligned: time and tokens.
    right: String,
    dimmed: bool,
}

fn row(row: Row, names: usize, width: usize) -> Line<'static> {
    let pointer = match row.chosen {
        true => Span::styled("❯ ", style::code()),
        false => Span::raw("  "),
    };
    let name_style = match (row.shown, row.dimmed) {
        (true, _) => style::bold(),
        (false, true) => style::dim(),
        (false, false) => Style::default(),
    };
    let indent = 2 * usize::from(row.indent);
    let name = cut(&printable(row.name), names.saturating_sub(indent));
    let pad = names.saturating_sub(text_width(&name) + indent);
    let used = 2 + 2 + names + 2;
    let right = match text_width(&row.right) + 2 <= width.saturating_sub(used + 12) {
        true => row.right,
        false => String::new(),
    };
    let room = width.saturating_sub(used + text_width(&right) + 2 * usize::from(!right.is_empty()));
    let detail = cut(&printable(&row.detail.replace('\n', " ")), room);
    let gap = room.saturating_sub(text_width(&detail)) + 2 * usize::from(!right.is_empty());
    let mut spans = vec![
        pointer,
        Span::raw(" ".repeat(indent)),
        row.glyph,
        Span::raw(" "),
        Span::styled(name, name_style),
        Span::raw(" ".repeat(pad + 2)),
        Span::styled(detail, style::dim()),
    ];
    if !right.is_empty() {
        spans.push(Span::raw(" ".repeat(gap)));
        spans.push(Span::styled(right, style::dim()));
    }
    Line::from(spans)
}

/// How an agent's task ended, as the interface says it: a style, a mark and a word for its row,
/// and what its transcript's line says it did.
pub fn ending(status: &str) -> (Style, &'static str, &'static str, &'static str) {
    match status {
        "achieved" | "unstated" => (style::done(), "✓", "done", "finished"),
        "not_achieved" => (
            style::failed(),
            "✗",
            "not achieved",
            "finished, not achieved",
        ),
        "exhausted" => (style::failed(), "✗", "out of turns", "ran out of turns"),
        "interrupted" => (style::waiting(), "■", "stopped", "was stopped"),
        _ => (style::failed(), "✗", "failed", "failed"),
    }
}

/// How an agent's last task ended, as a mark and a word.
fn ended(status: &str) -> (Span<'static>, String) {
    let (style, mark, word, _) = ending(status);
    (Span::styled(mark, style), word.into())
}

fn more(count: usize, arrow: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("  {arrow} {count} more"),
        style::dim(),
    ))
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
