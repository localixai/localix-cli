//! Test-only: an [`App`] driven as the person drives it (events and keys), and read back as the
//! terminal shows it. A frame goes through the real [`Screen`] into a fixed-size `vt100::Parser`,
//! and the tests assert on the rows a person would read. Spinners and durations are normalized,
//! so a snapshot does not depend on the moment it was drawn.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use protocol::{Label, Outcome};

use super::app::{Action, App};
use super::event::Event;
use super::live;
use super::screen::Screen;
use super::style;

/// A key pressed with no modifier.
pub(crate) fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

/// Ctrl and a letter.
pub(crate) fn ctrl(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

/// How a call reads, as the server labels it: the verb while it runs, once done, and its subject.
pub(crate) fn label(doing: &str, done: &str, subject: &str) -> Label {
    Label {
        doing: doing.into(),
        done: done.into(),
        subject: subject.into(),
    }
}

/// How a `bash` call of `command` reads.
pub(crate) fn ran(command: &str) -> Label {
    label("Running", "Ran", command)
}

/// Call `id` of `name` began.
pub(crate) fn call(id: &str, name: &str, label: &Label) -> Event {
    Event::Call {
        call: id.into(),
        name: name.into(),
        label: label.clone(),
    }
}

/// Call `id` of `name` ended with `outcome`.
pub(crate) fn result(id: &str, name: &str, label: &Label, outcome: Outcome) -> Event {
    Event::Result {
        call: id.into(),
        name: name.into(),
        label: label.clone(),
        outcome,
    }
}

/// A call that ended well with `report`.
pub(crate) fn done(report: &str) -> Outcome {
    Outcome::Done {
        report: report.into(),
    }
}

/// A call that failed, or that the person refused when `declined`.
pub(crate) fn failed(why: &str, declined: bool) -> Outcome {
    Outcome::Failed {
        why: why.into(),
        declined,
    }
}

/// Press `code` on `app`; what the person asked for, if anything.
pub(crate) fn press(app: &mut App, code: KeyCode, now: Instant) -> Option<Action> {
    app.event(key(code), now)
}

/// Type `text` a character at a time.
pub(crate) fn typed(app: &mut App, text: &str, now: Instant) {
    for c in text.chars() {
        app.event(key(KeyCode::Char(c)), now);
    }
}

/// What the session gets when it asks for its next entry now (as `Ui::next_entry` does): what
/// was queued, or `None` when nothing waits (the app then holds the question open, and an entry
/// that finds it already answered is queued instead).
pub(crate) fn next_entry(app: &mut App, now: Instant) -> Option<String> {
    let (reply, entries) = std::sync::mpsc::sync_channel(1);
    app.event(Event::Ready(reply), now);
    entries.try_recv().ok().flatten().map(|entry| entry.text)
}

/// The finished blocks not printed yet, as plain text; taking them prints them.
pub(crate) fn history(app: &mut App) -> String {
    plain(&app.take_history())
}

/// The live area at `now` (what runs, the composer or the question open), as plain text.
pub(crate) fn live_area(app: &App, now: Instant) -> String {
    plain(&live::render(app, now).0)
}

fn plain(lines: &[ratatui::text::Line<'static>]) -> String {
    lines
        .iter()
        .map(style::plain)
        .collect::<Vec<_>>()
        .join("\n")
}

/// A terminal of the app's size, drawn into as the interface's thread draws: what finished goes
/// above once, the live area is redrawn under it, and a changed view clears the screen first.
pub(crate) struct Term {
    screen: Screen<vt100::Parser>,
}

impl Term {
    pub(crate) fn new(app: &App) -> Self {
        let parser = vt100::Parser::new(
            u16::try_from(app.height).unwrap(),
            u16::try_from(app.width).unwrap(),
            0,
        );
        Self {
            screen: Screen::new(parser, false),
        }
    }

    /// One frame at `now`; the rows the terminal shows then, normalized.
    pub(crate) fn draw(&mut self, app: &mut App, now: Instant) -> String {
        if app.take_replay() {
            self.screen.reset().unwrap();
        }
        let history = app.take_history();
        let (live, cursor) = live::render(app, now);
        self.screen
            .frame(&history, &live, cursor, app.height, app.width)
            .unwrap();
        self.contents()
    }

    /// The rows the terminal shows, without trailing blanks or blank rows, normalized.
    pub(crate) fn contents(&self) -> String {
        let shown = self.screen.output().screen().contents();
        let rows: Vec<&str> = shown.lines().map(str::trim_end).collect();
        normalize(rows.join("\n").trim_end())
    }
}

/// `app` drawn once on a fresh terminal: its history not printed yet, then its live area.
pub(crate) fn render(app: &mut App, now: Instant) -> String {
    Term::new(app).draw(app, now)
}

/// Every spinner frame reads `*`, and every duration (`12s`, `3.0s`, `1m 05s`) reads with `N` in
/// place of its digits.
pub(crate) fn normalize(text: &str) -> String {
    const SPINNER: &str = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if SPINNER.contains(c) {
            out.push('*');
            at += 1;
            continue;
        }
        let starts = c.is_ascii_digit() && (at == 0 || !chars[at - 1].is_alphanumeric());
        if starts {
            let mut end = at;
            while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
                end += 1;
            }
            let unit = chars.get(end).is_some_and(|u| matches!(u, 's' | 'm' | 'h'));
            let alone = !chars.get(end + 1).is_some_and(|n| n.is_alphanumeric());
            if unit && alone && chars[end - 1] != '.' {
                out.push('N');
                at = end;
                continue;
            }
            out.extend(&chars[at..end]);
            at = end;
            continue;
        }
        out.push(c);
        at += 1;
    }
    out
}

/// The session's side of an interface no one draws: the test reads the events it is sent.
pub(crate) fn fake_ui() -> (super::Ui, std::sync::mpsc::Receiver<Event>) {
    #[allow(
        clippy::disallowed_methods,
        reason = "the interface's events are unbounded, as `tui::start` makes them"
    )]
    let (events, received) = std::sync::mpsc::channel();
    (super::Ui::new(events), received)
}
