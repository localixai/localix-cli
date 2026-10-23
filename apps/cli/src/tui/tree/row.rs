//! One row of the tree: a session's, by what it does or how it ended and what it spent, or a
//! job's; a pointer, a branch, a glyph, a name column and what is said after it.

use std::time::Instant;

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::{cells, ending, tokens};
use crate::tui::agents::{AgentState, Running, SessionView};
use crate::tui::printable::printable;
use crate::tui::style;
use crate::tui::tool_cell;
use crate::tui::wrap::{cut, width as text_width};

/// A session's row: a lane by what its task does or how its last one ended, an agent by its
/// state too (indented under its lane); on the right, how long it worked and its tokens (a
/// lane's count its agents').
pub(super) fn session_row<'a>(
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

/// What the calls running do: the first, and how many more.
fn doing(running: &[Running]) -> Option<String> {
    let first = tool_cell::doing(&running.first()?.label);
    Some(match running.len() {
        1 => first,
        n => format!("{first} +{} more", n - 1),
    })
}

/// How long the agent has worked and the tokens it used: `1m 08s · 54.1k tokens`.
fn spent(agent: &SessionView, now: Instant) -> String {
    let millis = agent.worked
        + agent.busy.map_or(0, |since| {
            u64::try_from(now.duration_since(since).as_millis()).unwrap_or(u64::MAX)
        });
    match (millis < 1000, agent.tokens) {
        (true, 0) => String::new(),
        (_, 0) => cells::clock(millis / 1000),
        (_, count) => format!("{} · {} tokens", cells::clock(millis / 1000), tokens(count)),
    }
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "a row's marks, each drawn on its own: any of them may hold with the others"
)]
pub(super) struct Row<'a> {
    /// The row the list's keys are on.
    pub(super) chosen: bool,
    /// An agent's, under its lane; the last of its lane's closes the branch.
    pub(super) branch: bool,
    pub(super) branch_last: bool,
    /// The session on the screen.
    pub(super) shown: bool,
    pub(super) glyph: Span<'static>,
    pub(super) name: &'a str,
    pub(super) detail: String,
    /// Time and tokens, after what it does.
    pub(super) right: String,
    pub(super) dimmed: bool,
}

pub(super) fn row(row: Row, names: usize, width: usize) -> Line<'static> {
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

/// How an agent's last task ended, as a mark and a word.
fn ended(status: &str) -> (Span<'static>, String) {
    let (style, mark, word, _) = ending(status);
    (Span::styled(mark, style), word.into())
}
