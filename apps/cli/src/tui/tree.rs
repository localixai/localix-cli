//! The sessions under the composer: the lane shown and the agents it started (none while it has
//! started none), a row each with what it does now or how its last task ended, how long it has
//! worked and the tokens it used, and the jobs. The agents hang under their lane as a branch, in a
//! name column of their own. The other lanes are one line; the ← list has them. While the list
//! has the keys (↓ from an empty prompt), `❯` marks the row chosen.
//!
//! ```text
//!   ⠹ fix the parser  Messaging agent tests · run the suite · 2m 03s · 81.2k tokens
//! ❯   ├ ⠹ scout  Reading src/lib.rs · 1m 12s · 54.1k tokens
//!     └ ✓ tests  done · 48s · 20.3k tokens
//!   ↑↓ select · →/enter open · x stop · esc input
//! ```
//!
//! With no agents and no jobs, only the line for the other lanes, if there are any:
//!
//! ```text
//!   ← 2 other sessions (1 working, 1 waiting for you)
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
/// Columns a lane's name takes at most, and an agent's or a job's.
const NAME: usize = 36;
const AGENT: usize = 20;
/// A lane whose question waits for the person, open or held until it is shown.
const WAITS: &str = "waiting for you";

/// The list's rows: the lane shown with its agents, and the jobs; the other lanes are one line.
/// None while there is nothing of the kind.
pub fn render(
    app: &App,
    now: Instant,
    spinner: &dyn Fn(Instant) -> Span<'static>,
) -> Vec<Line<'static>> {
    let sessions = &app.views.sessions;
    let order = app.views.order();
    let others = others(app);
    if order.is_empty() && app.views.jobs.is_empty() {
        return match others {
            Some(others) => vec![hint(&others, app.width)],
            None => vec![],
        };
    }
    let width = app.width.max(10);
    let column = |names: &mut dyn Iterator<Item = usize>, most: usize| {
        names.max().unwrap_or(0).clamp(1, most.min(width / 2))
    };
    // An agent's name column is its siblings', and the last of them closes the branch.
    let agents = |row: usize| {
        let names = (order.iter().skip(1)).map(|&at| text_width(sessions[at].title()));
        (
            column(&mut names.into_iter(), AGENT),
            row + 1 == order.len(),
        )
    };
    let focus = app.views.focus;
    let shown = order
        .iter()
        .position(|&at| sessions[at].id == app.views.shown);
    let line = |at_row: usize| {
        let view = &sessions[order[at_row]];
        let waits = app.waits_for_person(order[at_row]);
        let chosen = focus == Some(at_row);
        let mut row = session_row(view, chosen, shown == Some(at_row), now, spinner);
        if waits {
            (row.glyph, row.detail) = (Span::styled("?", style::waiting()), WAITS.into());
        }
        // The lane is the one on the screen anyway; an agent shown is marked.
        if row.shown && row.branch {
            row.detail = format!("current · {}", row.detail);
        }
        let (names, last) = match row.branch {
            true => agents(at_row),
            false => (
                column(&mut std::iter::once(text_width(view.title())), NAME),
                false,
            ),
        };
        row.branch_last = last;
        self::row(row, names, width)
    };
    let mut out = vec![];
    if !order.is_empty() {
        out.push(line(0));
        // A window of the agents around the row chosen, else the one shown.
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
    }
    let names = column(
        &mut app.views.jobs.iter().map(|job| text_width(&job.name)),
        AGENT,
    );
    out.extend(jobs(app, order.len(), names, width));
    let on_agent = app.shown().parent.is_some();
    let others = others.unwrap_or_else(|| "← sessions".into());
    let hint = match (focus, on_agent) {
        (Some(_), _) => "↑↓ select · →/enter open · x stop · esc input".to_string(),
        (None, true) => format!("↓ to manage · esc back to {}", app.home_title()),
        (None, false) => format!("↓ to manage · {others}"),
    };
    out.push(self::hint(&hint, width));
    out
}

/// The lanes not shown, counted, with those that need a look: `← 2 other sessions (1 working)`.
fn others(app: &App) -> Option<String> {
    let home = app.views.home();
    let sessions = &app.views.sessions;
    let lanes: Vec<usize> = (0..sessions.len())
        .filter(|&at| at != home && sessions[at].lane.is_some())
        .collect();
    if lanes.is_empty() {
        return None;
    }
    let count = |test: &dyn Fn(usize) -> bool| lanes.iter().filter(|&&at| test(at)).count();
    let working = count(&|at| sessions[at].busy.is_some());
    let waiting = count(&|at| app.waits_for_person(at));
    let finished = count(&|at| sessions[at].unseen && sessions[at].busy.is_none());
    let notes: Vec<String> = [
        (working, "working"),
        (waiting, WAITS),
        (finished, "finished"),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, what)| format!("{n} {what}"))
    .collect();
    let s = if lanes.len() == 1 { "" } else { "s" };
    let mut said = format!("← {} other session{s}", lanes.len());
    if !notes.is_empty() {
        said.push_str(&format!(" ({})", notes.join(", ")));
    }
    Some(said)
}

fn hint(text: &str, width: usize) -> Line<'static> {
    Line::from(Span::styled(
        format!("  {}", cut(text, width.saturating_sub(2))),
        style::dim(),
    ))
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
        branch: view.parent.is_some(),
        branch_last: false,
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
                branch: false,
                branch_last: false,
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
    match (millis < 1000, agent.tokens) {
        (true, 0) => String::new(),
        (_, 0) => cells::clock(millis / 1000),
        (_, count) => format!("{} · {} tokens", cells::clock(millis / 1000), tokens(count)),
    }
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
    /// An agent's, under its lane; the last of its lane's closes the branch.
    branch: bool,
    branch_last: bool,
    /// The session on the screen.
    shown: bool,
    glyph: Span<'static>,
    name: &'a str,
    detail: String,
    /// Time and tokens, after what it does.
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
    let branch = match (row.branch, row.branch_last) {
        (false, _) => "",
        (true, false) => "  ├ ",
        (true, true) => "  └ ",
    };
    let name = cut(&printable(row.name), names);
    let pad = names.saturating_sub(text_width(&name));
    let detail = printable(&row.detail.replace('\n', " "));
    let said = match row.right.is_empty() {
        true => detail,
        false => format!("{detail} · {}", row.right),
    };
    let room = width.saturating_sub(2 + text_width(branch) + 2 + names + 2);
    Line::from(vec![
        pointer,
        Span::styled(branch, style::dim()),
        row.glyph,
        Span::raw(" "),
        Span::styled(name, name_style),
        Span::raw(" ".repeat(pad + 2)),
        Span::styled(cut(&said, room), style::dim()),
    ])
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
