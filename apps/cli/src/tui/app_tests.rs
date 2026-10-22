use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use contracts::tool::ToolOutput;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use super::*;
use crate::tui::style;

fn app() -> App {
    let commands = [
        ("help", "the commands"),
        ("mode", "show or switch the mode"),
        ("new", "start over"),
    ];
    App::new(
        60,
        20,
        commands
            .iter()
            .map(|(n, w)| (n.to_string(), w.to_string()))
            .collect(),
    )
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn ctrl(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

fn typed(app: &mut App, text: &str, now: Instant) {
    for c in text.chars() {
        app.event(key(KeyCode::Char(c)), now);
    }
}

fn history(app: &mut App) -> String {
    app.take_history()
        .iter()
        .map(style::plain)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn an_entry_goes_to_the_session_when_it_waits_and_is_queued_while_it_works() {
    let now = Instant::now();
    let mut app = app();
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    typed(&mut app, "first", now);
    app.event(key(KeyCode::Enter), now);
    assert_eq!(entries.try_recv(), Ok(Some("first".into())));
    app.event(Event::TaskStarted("first".into()), now);
    typed(&mut app, "second", now);
    app.event(key(KeyCode::Enter), now);
    assert_eq!(app.queued, [Typed::from("second")]);
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    assert_eq!(entries.try_recv(), Ok(Some("second".into())));
    assert!(app.queued.is_empty());
}

#[test]
fn a_task_leaves_its_blocks_in_order() {
    let now = Instant::now();
    let mut app = app();
    app.event(Event::TaskStarted("list the files".into()), now);
    let args = json!({"command": "ls"});
    app.event(
        Event::Call {
            name: "bash".into(),
            args: args.clone(),
        },
        now,
    );
    assert!(app.running.is_some());
    let outcome = Ok(ToolOutput {
        report: "exit 0 in 0.0s\n### stdout\nREADME.md".into(),
        state: None,
        image: None,
    });
    app.event(
        Event::Result {
            name: "bash".into(),
            args,
            outcome,
        },
        now,
    );
    app.event(Event::Answer("One file: `README.md`.".into()), now);
    app.event(
        Event::TaskEnded {
            status: "achieved".into(),
            millis: 3_000,
            turns: 2,
            cost: 0.001,
        },
        now,
    );
    insta::assert_snapshot!(history(&mut app), @r"
    ❯ list the files

    • Ran ls
      └ README.md

    • One file: README.md.

    ─ Worked for 3.0s · 2 turns · $0.0010 ──────────────────────
    ");
    assert!(app.busy.is_none() && app.running.is_none());
}

#[test]
fn ctrl_c_stops_a_task_clears_the_line_and_quits_only_when_pressed_twice() {
    let now = Instant::now();
    let mut app = app();
    app.event(Event::TaskStarted("x".into()), now);
    assert_eq!(app.event(ctrl('c'), now), Some(Action::Stop));
    assert_eq!(app.event(key(KeyCode::Esc), now), None, "stopping already");
    app.event(
        Event::TaskEnded {
            status: "interrupted".into(),
            millis: 0,
            turns: 0,
            cost: 0.0,
        },
        now,
    );
    typed(&mut app, "draft", now);
    assert_eq!(app.event(ctrl('c'), now), None);
    assert!(app.composer.is_empty());
    assert_eq!(app.event(ctrl('c'), now), None);
    assert_eq!(
        app.event(ctrl('c'), now + Duration::from_secs(3)),
        None,
        "too late: armed again"
    );
    assert_eq!(
        app.event(ctrl('c'), now + Duration::from_secs(4)),
        Some(Action::Quit)
    );
    assert_eq!(app.event(ctrl('d'), now), Some(Action::Quit));
}

#[test]
fn an_approval_is_answered_with_a_key() {
    let now = Instant::now();
    let mut app = app();
    let (reply, answer) = channel();
    app.event(
        Event::Approve {
            tool: "bash".into(),
            summary: "run cargo test".into(),
            reply,
        },
        now,
    );
    app.event(key(KeyCode::Char('n')), now);
    typed(&mut app, "use make", now);
    app.event(key(KeyCode::Enter), now);
    assert_eq!(answer.try_recv(), Ok(Approval::No(Some("use make".into()))));
    assert!(app.select.is_none());
    let (reply, answer) = channel();
    app.event(
        Event::Approve {
            tool: "bash".into(),
            summary: "rm -rf build".into(),
            reply,
        },
        now,
    );
    assert_eq!(app.event(ctrl('c'), now), Some(Action::Stop));
    assert_eq!(answer.try_recv(), Ok(Approval::No(None)));
}

#[test]
fn slash_offers_the_commands_and_enter_takes_the_marked_one() {
    let now = Instant::now();
    let mut app = app();
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    typed(&mut app, "/", now);
    assert_eq!(app.offers().len(), 3);
    typed(&mut app, "n", now);
    assert_eq!(app.offers().len(), 1);
    app.event(key(KeyCode::Enter), now);
    assert_eq!(entries.try_recv(), Ok(Some("/new".into())));
    typed(&mut app, "/m", now);
    app.event(key(KeyCode::Tab), now);
    assert_eq!(app.composer.text(), "/mode");
    typed(&mut app, " plan", now);
    assert!(app.offers().is_empty(), "arguments end the offer");
}

#[test]
fn leaving_ends_every_wait() {
    let now = Instant::now();
    let mut app = app();
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    app.leave();
    assert_eq!(entries.try_recv(), Ok(None));
}

#[test]
fn a_call_the_person_refused_says_what_they_said_not_an_error() {
    let now = Instant::now();
    let mut app = app();
    let args = json!({"command": "git log"});
    app.event(
        Event::Call {
            name: "bash".into(),
            args: args.clone(),
        },
        now,
    );
    let (reply, _answer) = channel();
    app.event(
        Event::Approve {
            tool: "bash".into(),
            summary: "git log".into(),
            reply,
        },
        now,
    );
    app.event(key(KeyCode::Char('n')), now);
    typed(&mut app, "not this repository", now);
    app.event(key(KeyCode::Enter), now);
    let outcome = Err(contracts::tool::ToolError::Failed(
        "not allowed: the person declined it".into(),
    ));
    app.event(
        Event::Result {
            name: "bash".into(),
            args,
            outcome,
        },
        now,
    );
    let lines = app.take_history();
    assert_eq!(lines[0].spans[0].style, style::waiting());
    insta::assert_snapshot!(lines.iter().map(style::plain).collect::<Vec<_>>().join("\n"), @r"
    • Skipped git log
      └ You said no: not this repository
    ");
}

#[test]
fn at_offers_files_and_enter_puts_the_path_in_without_sending() {
    let now = Instant::now();
    let mut app = app();
    app.files = crate::tui::mention::Files::with(vec!["src/main.rs".into(), "README.md".into()]);
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    typed(&mut app, "explain @mai", now);
    assert_eq!(
        app.offers(),
        [crate::tui::keys::Offer::File("src/main.rs".into())]
    );
    app.event(key(KeyCode::Enter), now);
    assert_eq!(app.composer.text(), "explain src/main.rs ");
    assert!(entries.try_recv().is_err(), "nothing was sent");
}

#[test]
fn dropped_files_become_marks_and_go_with_the_message() {
    let now = Instant::now();
    let dir = crate::testing::Scratch::new();
    let picture = dir.path().join("Screenshot at 2.26.05\u{202f}AM.png");
    std::fs::write(&picture, b"\x89PNG\r\n\x1a\n").unwrap();
    let notes = dir.path().join("notes.md");
    std::fs::write(&notes, "# notes").unwrap();
    let mut app = app();
    app.pasted = dir.path().join("kept");
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    typed(&mut app, "compare ", now);
    let dropped = format!(
        "{} {}",
        picture.display().to_string().replace(' ', "\\ "),
        notes.display()
    );
    app.event(Event::Paste(dropped), now);
    assert_eq!(
        app.composer.text(),
        "compare [image: Screenshot at 2.26.05\u{202f}AM.png] [file: notes.md]"
    );
    app.event(key(KeyCode::Enter), now);
    let sent = entries.try_recv().unwrap().unwrap();
    assert_eq!(
        sent.text,
        format!(
            "compare [image: Screenshot at 2.26.05\u{202f}AM.png] {}",
            notes.display()
        )
    );
    assert_eq!(sent.images.len(), 1);
    assert!(
        sent.images[0].starts_with(dir.path().join("kept")),
        "a kept copy is sent"
    );
}
