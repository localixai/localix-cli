use std::time::Instant;

use crossterm::event::KeyCode;

use crate::tui::app::{Action, App};
use crate::tui::event::{Event, JobRow};
use crate::tui::test_support::{ctrl, in_session as from, press, render, typed};

/// Lanes A (shown) and B; B works when `b_works`.
fn two(b_works: bool, now: Instant) -> App {
    let mut app = App::new(100, 30, vec![]);
    for (id, name) in [("A", "first"), ("B", "fix the flaky jobs test")] {
        let (id, name) = (id.to_string(), name.to_string());
        app.event(Event::LaneStarted { id, name }, now);
    }
    if b_works {
        app.frame(from("B", Event::TaskStarted("fix it".into())), now);
    }
    app
}

/// Ten minutes from now, in milliseconds since the epoch, as the wire gives a deadline.
fn in_ten_minutes() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    u64::try_from(now.unwrap().as_millis()).unwrap() + 600_000
}

fn monitor() -> JobRow {
    JobRow {
        id: "job-1".into(),
        name: "watch-ci".into(),
        detail: "gh run watch".into(),
        until_at: Some(in_ten_minutes()),
        next: None,
        events: 0,
        lane: String::new(),
    }
}

#[test]
fn second_ctrl_c_with_others_busy_asks_to_quit() {
    let now = Instant::now();
    let mut app = two(true, now);
    app.shells = Box::new(|| 3);
    app.frame(from("A", Event::Jobs(vec![monitor()])), now);
    assert_eq!(app.input(ctrl('c'), now), None);
    assert_eq!(app.input(ctrl('c'), now), None, "asked first");
    let shown = render(&mut app, now);
    assert!(shown.contains("• Quit llx?"), "{shown}");
    assert!(
        shown.contains(
            "1 session still working · 3 shells and 1 monitor running — they will be stopped."
        ),
        "{shown}"
    );
    assert!(
        shown.contains("Quit anyway") && shown.contains("Stay"),
        "{shown}"
    );
    press(&mut app, KeyCode::Down, now);
    assert_eq!(press(&mut app, KeyCode::Enter, now), None, "Stay");
    assert!(app.questions.is_empty());
    app.input(ctrl('c'), now);
    app.input(ctrl('c'), now);
    assert_eq!(press(&mut app, KeyCode::Enter, now), Some(Action::QuitNow));
}

#[test]
fn ctrl_d_with_nothing_busy_quits() {
    let now = Instant::now();
    let mut app = two(false, now);
    assert_eq!(app.input(ctrl('d'), now), Some(Action::Quit));
}

#[test]
fn ctrl_d_with_a_shell_running_asks() {
    let now = Instant::now();
    let mut app = two(false, now);
    app.shells = Box::new(|| 1);
    assert_eq!(app.input(ctrl('d'), now), None);
    let shown = render(&mut app, now);
    assert!(
        shown.contains("1 shell running — it will be stopped."),
        "{shown}"
    );
}

#[test]
fn exit_with_a_session_working_asks() {
    let now = Instant::now();
    let mut app = two(true, now);
    typed(&mut app, "/exit", now);
    assert_eq!(press(&mut app, KeyCode::Enter, now), None);
    let shown = render(&mut app, now);
    assert!(shown.contains("• Quit llx?"), "{shown}");
    assert_eq!(press(&mut app, KeyCode::Esc, now), None, "Esc stays");
    assert!(app.questions.is_empty());
}

#[test]
fn ctrl_c_while_stopping_quits_only_when_nothing_else_works() {
    let now = Instant::now();
    let mut alone = two(false, now);
    alone.frame(from("A", Event::TaskStarted("a's task".into())), now);
    assert_eq!(alone.input(ctrl('c'), now), Some(Action::Stop("A".into())));
    assert_eq!(
        alone.input(ctrl('c'), now),
        Some(Action::Stop("A".into())),
        "stopped again, it quits, as before"
    );
    let mut app = two(true, now);
    app.frame(from("A", Event::TaskStarted("a's task".into())), now);
    assert_eq!(app.input(ctrl('c'), now), Some(Action::Stop("A".into())));
    assert_eq!(app.input(ctrl('c'), now), None, "B works: asked first");
    assert!(render(&mut app, now).contains("• Quit llx?"));
}
