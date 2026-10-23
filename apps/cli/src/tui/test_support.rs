//! Test-only: an [`App`] driven as the person drives it (events and keys), and read back as the
//! terminal shows it. A frame goes through the real [`Screen`] into a fixed-size `vt100::Parser`,
//! and the tests assert on the rows a person would read. Spinners and durations are normalized,
//! so a snapshot does not depend on the moment it was drawn.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use protocol::{Label, Outcome};

use protocol::{Asked, Command, Input as Sent};

use super::app::{Action, App};
use super::event::{Event, Input, Out};
use super::live;
use super::screen::Screen;
use super::style;

/// A key pressed with no modifier.
pub(crate) fn key(code: KeyCode) -> Input {
    Input::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

/// Ctrl and a letter.
pub(crate) fn ctrl(c: char) -> Input {
    Input::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
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
    app.input(key(code), now)
}

/// Type `text` a character at a time.
pub(crate) fn typed(app: &mut App, text: &str, now: Instant) {
    for c in text.chars() {
        app.input(key(KeyCode::Char(c)), now);
    }
}

/// What the person sent the server so far, taken: each entry as `Command::Input` carries it.
pub(crate) fn sent(app: &mut App) -> Vec<Sent> {
    let mut sent = vec![];
    app.outbox.retain(|out| match out {
        Out::Call(Command::Input(input)) => {
            sent.push(input.clone());
            false
        }
        _ => true,
    });
    sent
}

/// The texts of what the person sent, taken.
pub(crate) fn sent_texts(app: &mut App) -> Vec<String> {
    sent(app).into_iter().map(|input| input.text).collect()
}

/// The replies the person gave so far, taken, each with its question's id.
pub(crate) fn replies(app: &mut App) -> Vec<(u64, protocol::Reply)> {
    let mut replies = vec![];
    app.outbox.retain(|out| match out {
        Out::Reply(id, reply) => {
            replies.push((*id, reply.clone()));
            false
        }
        Out::Call(_) => true,
    });
    replies
}

/// The modes, each with what it does, as the server says them.
pub(crate) fn modes() -> Vec<(String, String)> {
    [
        (
            "default",
            "Each command, file change and browser action asks first",
        ),
        ("plan", "Investigate and plan: file changes are refused"),
        (
            "bypass",
            "Everything runs without asking (-y); questions are asked",
        ),
        (
            "autonomous",
            "Everything runs without asking, and no question is asked",
        ),
    ]
    .map(|(n, w)| (n.to_string(), w.to_string()))
    .to_vec()
}

/// The commands the interface sent so far other than entries, taken.
pub(crate) fn commands(app: &mut App) -> Vec<Command> {
    let mut commands = vec![];
    app.outbox.retain(|out| match out {
        Out::Call(Command::Input(_)) | Out::Reply(..) => true,
        Out::Call(command) => {
            commands.push(command.clone());
            false
        }
    });
    commands
}

/// Whether an effect of `tool` may happen, as the server asks it.
pub(crate) fn approve(tool: &str, summary: &str) -> Asked {
    Asked::Approve {
        tool: tool.into(),
        summary: summary.into(),
    }
}

/// The lane's queue as the server shows it: whether it waits, the words steered, the entries
/// queued, what came back from the background.
pub(crate) fn queue(ready: bool, steered: &[&str], queued: &[&str], background: &[&str]) -> Event {
    let owned = |texts: &[&str]| texts.iter().map(|t| (*t).to_string()).collect();
    Event::Queue {
        ready,
        steered: owned(steered),
        queued: owned(queued),
        background: owned(background),
    }
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

/// `event` as session `id` (a lane or an agent) sends it.
pub(crate) fn in_session(id: &str, event: Event) -> protocol::ToClient {
    protocol::ToClient::Event {
        session: Some(id.into()),
        event,
    }
}

/// A directory of a test's own, removed when it is dropped.
pub(crate) struct Scratch(std::path::PathBuf);

impl Scratch {
    pub(crate) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let name = format!("llx-tui-{}-{n}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
