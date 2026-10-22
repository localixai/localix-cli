use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use super::*;
use crate::tui::event::Event;
use crate::tui::style;

fn shown(app: &App, now: Instant) -> String {
    let (lines, _) = render(app, now);
    lines
        .iter()
        .map(style::plain)
        .collect::<Vec<_>>()
        .join("\n")
}

fn app() -> App {
    App::new(
        60,
        20,
        vec![
            ("mode".into(), "show or switch the mode".into()),
            ("model".into(), "pick the model".into()),
        ],
    )
}

#[test]
fn idle_is_the_prompt_alone() {
    let app = app();
    let (lines, cursor) = render(&app, Instant::now());
    assert_eq!(
        lines.iter().map(style::plain).collect::<Vec<_>>(),
        ["❯ Type a task, / for commands"]
    );
    assert_eq!(cursor, Some(Cursor { row: 0, column: 2 }));
}

#[test]
fn working_shows_the_call_its_steps_the_time_and_the_queue() {
    let start = Instant::now();
    let mut app = app();
    app.event(Event::TaskStarted("check the shop".into()), start);
    app.event(
        Event::Call {
            name: "browser".into(),
            args: json!({"url": "http://127.0.0.1:4173/"}),
        },
        start,
    );
    app.event(Event::Step("click \"Buy\" [e12]".into()), start);
    app.queued.push_back("then update the changelog".into());
    app.mode = "plan".into();
    insta::assert_snapshot!(shown(&app, start + Duration::from_millis(12_300)), @r#"
    ⠸ Browsing http://127.0.0.1:4173/
      └ click "Buy" [e12]

    ⠸ Working (12s · esc to interrupt)
      ↳ then update the changelog

    ❯ Type a task, / for commands
      plan mode · shift+tab to leave
    "#);
}

#[test]
fn a_question_takes_the_composer_place() {
    let now = Instant::now();
    let mut app = app();
    app.event(Event::TaskStarted("x".into()), now);
    let (reply, _answer) = channel();
    app.event(
        Event::Approve {
            tool: "bash".into(),
            summary: "cargo test --workspace".into(),
            reply,
        },
        now,
    );
    insta::assert_snapshot!(shown(&app, now), @r"
    • Run this command?
      cargo test --workspace

    ❯ 1. Yes
      2. Yes, and don't ask again for bash
      3. No, and tell llx what to do differently

      ↑↓ to choose · enter to confirm · esc to say no
    ");
}

#[test]
fn slash_lists_the_commands_under_the_prompt() {
    let now = Instant::now();
    let mut app = app();
    app.event(
        Event::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE)),
        now,
    );
    insta::assert_snapshot!(shown(&app, now), @r"
    ❯ /
      /mode       show or switch the mode
      /model      pick the model
    ");
}
