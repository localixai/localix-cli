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
use super::cells::clock;
use super::keys::Offer;
use super::screen::Cursor;
use super::style;
use super::tool_cell::{self, State};
use super::tree;
use super::wrap::cut;
use crate::tui::printable::printable;

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const FRAME_MS: u128 = 80;
const PLACEHOLDER: &str = "Type a task, / for commands";

/// The live rows at `now`, and where the cursor goes among them.
pub(crate) fn render(app: &App, now: Instant) -> (Vec<Line<'static>>, Option<Cursor>) {
    let width = app.width.max(10);
    let spinner = |since: Instant| {
        let frame = now.duration_since(since).as_millis() / FRAME_MS;
        Span::styled(
            SPINNER[frame as usize % SPINNER.len()].to_string(),
            style::code(),
        )
    };
    let (mut out, busy) = running_rows(app, width, &spinner);
    if let Some(started) = busy.filter(|_| app.questions.is_empty()) {
        let took = clock(now.duration_since(started).as_secs());
        let state = if app.lane().stopping {
            "stopping"
        } else if app.shown().parent.is_some() {
            "ctrl-c to interrupt"
        } else {
            "esc to interrupt"
        };
        out.push(Line::from(vec![
            spinner(started),
            Span::raw(" "),
            Span::styled("Working", style::bold()),
            Span::styled(format!(" ({took} · {state})"), style::dim()),
        ]));
    }
    out.extend(waiting_rows(app, width));
    if !out.is_empty() && out.last() != Some(&Line::default()) {
        out.push(Line::default());
    }
    if let Some(asked) = app.questions.front() {
        let (rows, cursor) = asked.select.render(width);
        let top = out.len();
        out.extend(rows);
        let more = app.questions.len() - 1;
        if more > 0 {
            let s = if more == 1 { "" } else { "s" };
            let waiting = format!("  {more} more question{s} waiting");
            out.push(Line::from(Span::styled(waiting, style::dim())));
        }
        let cursor = cursor.map(|(row, column)| Cursor {
            row: top + row,
            column,
        });
        return (out, cursor);
    }
    if app.listing.is_some() {
        out.extend(app.sessions_render(now, &spinner));
        return (out, None);
    }
    let prompt = Span::styled("❯ ", style::bold());
    let shown = app.shown();
    let viewing = shown.parent.as_ref().map(|_| {
        let lane = app.home_title();
        format!(
            "Viewing agent {} · a task you type goes to {lane}",
            shown.name
        )
    });
    let placeholder = viewing.as_deref().unwrap_or(PLACEHOLDER);
    let (rows, (row, column)) = app.composer.render(&prompt, placeholder, width);
    let cursor = Cursor {
        row: out.len() + row,
        column,
    };
    out.extend(rows);
    let offers = app.offers();
    if let Some(context) = command_context(&app.composer.text(), &app.lane().mode) {
        out.push(Line::from(Span::styled(
            format!("  {}", cut(&context, width.saturating_sub(2))),
            style::dim(),
        )));
    }
    out.extend(offer_rows(app, &offers, width));
    let footer = footer(app, now);
    let spent = app.lane().total.as_ref().map(super::spend::spent);
    let row = width.saturating_sub(2);
    let footer = super::spend::across(&cut(&footer, row), &spent.unwrap_or_default(), row);
    out.push(Line::from(Span::styled(
        format!("  {footer}"),
        style::dim(),
    )));
    if offers.is_empty() {
        let tree = tree::render(app, now, &spinner);
        if !tree.is_empty() {
            out.push(Line::default());
            out.extend(tree);
        }
    }
    (out, Some(cursor))
}

/// The calls running in the view shown, a spinner each, the hands' steps under the last; and
/// when its task started.
fn running_rows(
    app: &App,
    width: usize,
    spinner: &dyn Fn(Instant) -> Span<'static>,
) -> (Vec<Line<'static>>, Option<Instant>) {
    let mut out: Vec<Line<'static>> = vec![];
    let (running, busy) = app.shown_work();
    // While a question is open it says what the call would do; the calls show again after.
    let running = if app.questions.is_empty() {
        running
    } else {
        &[]
    };
    for (at, call) in running.iter().enumerate() {
        let mut rows = tool_cell::lines(&call.name, &call.label, &State::Running, width);
        if let Some(first) = rows.first_mut() {
            first.spans[0] = spinner(call.since);
            first.spans.insert(1, Span::raw(" "));
        }
        out.extend(rows);
        // The hands' steps belong to the main session's last call.
        let last = at + 1 == running.len();
        let steps = match (&app.shown().lane, last) {
            (Some(lane), true) => &lane.steps[..],
            _ => &[],
        };
        for step in steps {
            let step = cut(&printable(step), width.saturating_sub(4));
            out.push(Line::from(Span::styled(
                format!("  └ {step}"),
                style::dim(),
            )));
        }
    }
    if !running.is_empty() {
        out.push(Line::default());
    }
    (out, busy)
}

/// What waits, a row each.
fn waiting_rows(app: &App, width: usize) -> Vec<Line<'static>> {
    // Sent to the task, waiting for its next step; then what waits for the task to end, as the
    // transcript will show it (the background's by where it came from, `≋ ticker · …`).
    let lane = app.lane();
    let waiting = lane
        .steered
        .iter()
        .map(|text| format!("↳ {}", waiting(text)))
        .chain(
            lane.queued
                .iter()
                .map(|text| format!("⏎ {}", waiting(text))),
        )
        .chain(lane.background.iter().cloned());
    let rows = waiting.map(|entry| {
        let entry = cut(&printable(&entry.replace('\n', " ")), width - 2);
        Line::from(Span::styled(format!("  {entry}"), style::dim()))
    });
    rows.collect()
}

/// What `/` or `@` offers, the one marked in bold.
fn offer_rows(app: &App, offers: &[Offer], width: usize) -> Vec<Line<'static>> {
    let mut out = vec![];
    for (at, offer) in offers.iter().enumerate() {
        let marked = at == app.offered.min(offers.len() - 1);
        let name_style = if marked { style::bold() } else { style::dim() };
        // Cut, not wrapped: the names stay in a column.
        let row = |label: String, what: &str| {
            let label = cut(&label, width);
            let room = width.saturating_sub(crate::tui::wrap::width(&label));
            Line::from(vec![
                Span::styled(label, name_style),
                Span::from(if room > 0 { " " } else { "" }),
                Span::styled(cut(&printable(what), room.saturating_sub(1)), style::dim()),
            ])
        };
        let line = match offer {
            Offer::Command(name, what) => row(format!("  /{name:<10}"), what),
            Offer::Mode(name, what) => row(format!("  {name:<12}"), what),
            Offer::File(path) => Line::from(Span::styled(
                format!("  {}", cut(&printable(path), width.saturating_sub(2))),
                name_style,
            )),
        };
        out.push(line);
    }
    out
}

fn command_context(text: &str, current: &str) -> Option<String> {
    match text {
        "/mode" | "/permissions" => Some(format!("mode  ›  current: {current} · enter to list")),
        "/plan" => Some("plan  ›  enter to switch · tab to add a task".into()),
        "/mode " | "/permissions " => Some(format!(
            "mode  ›  current: {current} · ↑↓ choose · tab complete · enter apply"
        )),
        _ if text.starts_with("/mode ") || text.starts_with("/permissions ") => Some(format!(
            "mode  ›  current: {current} · ↑↓ choose · tab complete · enter apply"
        )),
        _ if text.starts_with("/plan ") => {
            Some("plan  ›  type a task · enter to run in plan mode".into())
        }
        _ => None,
    }
}

/// An entry that waits: a key's by what it does, not the sequence it sent.
fn waiting(text: &str) -> String {
    use protocol::{LESS_REASONING, MORE_REASONING, SHIFT_TAB};
    match text {
        SHIFT_TAB => "shift+tab · next mode".into(),
        MORE_REASONING => "more reasoning".into(),
        LESS_REASONING => "less reasoning".into(),
        text => text.to_string(),
    }
}

fn footer(app: &App, now: Instant) -> String {
    let window = |at: Instant| now.duration_since(at).as_millis() < super::app::QUIT_WINDOW_MS;
    let mode = format!("{} mode · shift+tab to cycle", app.lane().mode);
    if app.quitting.is_some_and(window) {
        let back = if app.discarded {
            "↑ brings it back · "
        } else {
            ""
        };
        // In place of the mode's line while it lasts: a short window to read it in.
        return format!("{back}ctrl-c again to quit");
    }
    if app.escaped.is_some_and(window) && app.composer.is_empty() && app.busy().is_none() {
        return "esc again to rewind".into();
    }
    if app.busy().is_some() && !app.composer.is_empty() {
        return format!("{mode} · enter tells the task now · tab keeps it for after");
    }
    // How full the context is, once it matters: below half left.
    let context = app
        .lane()
        .context
        .filter(|&left| left < 50)
        .map(|left| format!("{left}% context left"));
    match context {
        Some(context) => format!("{mode} · {context}"),
        None => mode,
    }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
