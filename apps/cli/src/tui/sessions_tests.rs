use std::time::Instant;

use crossterm::event::KeyCode;

use crate::desk::Target;
use crate::tui::app::{Action, App};
use crate::tui::event::Event;
use crate::tui::test_support::{Term, press, typed};

/// `event` as session `id` sends it.
fn from(id: &str, event: Event) -> Event {
    Event::In(id.into(), Box::new(event))
}

/// Lanes A (shown, idle, one task done), B (working) and C (a choice held for it).
fn three(now: Instant) -> App {
    let mut app = App::new(80, 30, vec![]);
    for (id, name) in [
        ("A", "refactor the picker"),
        ("B", "fix the flaky jobs test"),
    ] {
        let (id, name) = (id.to_string(), name.to_string());
        app.event(Event::LaneStarted { id, name }, now);
    }
    let (id, name) = ("C".to_string(), "add tabs to the browser".to_string());
    app.event(Event::LaneStarted { id, name }, now);
    app.event(from("A", Event::Note("A's own transcript".into())), now);
    app.event(from("A", Event::TaskStarted("refactor it".into())), now);
    let ended = Event::TaskEnded {
        status: "achieved".into(),
        millis: 1000,
        turns: 1,
        cost: 0.0,
    };
    app.event(from("A", ended), now);
    app.event(from("B", Event::Note("B's own transcript".into())), now);
    app.event(from("B", Event::TaskStarted("fix it".into())), now);
    let (reply, picked) = std::sync::mpsc::channel();
    std::mem::forget(picked);
    let choose = Event::Choose {
        title: "Implement this plan?".into(),
        options: vec![("Yes".into(), String::new())],
        marked: None,
        reply,
    };
    app.event(from("C", choose), now);
    app
}

fn saved() -> Event {
    let row = |id: &str, title: &str, meta: &str| (id.into(), title.into(), meta.into());
    Event::Saved(vec![
        row("B", "fix the flaky jobs test", "2 h ago · 4 tasks"),
        row("old", "make the binary smaller", "1 d ago · 1 task"),
    ])
}

#[test]
fn left_on_empty_composer_asks_for_sessions() {
    let now = Instant::now();
    let mut app = three(now);
    assert_eq!(press(&mut app, KeyCode::Left, now), Some(Action::Sessions));
    press(&mut app, KeyCode::Esc, now);
    typed(&mut app, "ab", now);
    assert_eq!(
        press(&mut app, KeyCode::Left, now),
        None,
        "it moves the cursor"
    );
    let screen = Term::new(&app).draw(&mut app, now);
    assert!(!screen.contains("Sessions in this project"), "{screen}");
    app.composer.clear();
    typed(&mut app, "/", now);
    assert_eq!(press(&mut app, KeyCode::Left, now), None, "with offers");
}

#[test]
fn saved_rows_open_the_list_live_first() {
    let now = Instant::now();
    let mut app = three(now);
    let mut term = Term::new(&app);
    term.draw(&mut app, now);
    press(&mut app, KeyCode::Left, now);
    app.event(saved(), now);
    let screen = term.draw(&mut app, now);
    let list = screen.find("• Sessions").map_or("", |at| &screen[at..]);
    insta::assert_snapshot!(list, @"
    • Sessions in this project

      * fix the flaky jobs test  running
    ❯ ? add tabs to the browser  waiting for you
      ✓ refactor the picker      current · done · Ns · 1 task
      + New session

      Earlier
        make the binary smaller  1 d ago · 1 task

      ↑↓ choose · enter open · x stop · esc back
    ");
}

#[test]
fn choosing_a_live_row_shows_it() {
    let now = Instant::now();
    let mut app = three(now);
    let mut term = Term::new(&app);
    term.draw(&mut app, now);
    press(&mut app, KeyCode::Left, now);
    // The cursor starts on the newest live session not shown: C, held; B is above it.
    press(&mut app, KeyCode::Up, now);
    assert_eq!(press(&mut app, KeyCode::Enter, now), None);
    assert_eq!(app.views.shown, "B");
    let screen = term.draw(&mut app, now);
    assert!(
        screen.trim_start().starts_with("B's own transcript"),
        "{screen}"
    );
    assert!(!screen.contains("A's own transcript"), "{screen}");
    assert!(!screen.contains("Sessions in this project"), "{screen}");
}

#[test]
fn choosing_a_saved_row_opens_it() {
    let now = Instant::now();
    let mut app = three(now);
    press(&mut app, KeyCode::Left, now);
    app.event(saved(), now);
    for _ in 0..3 {
        press(&mut app, KeyCode::Down, now);
    }
    let opened = press(&mut app, KeyCode::Right, now);
    assert_eq!(opened, Some(Action::Open(Target::Saved("old".into()))));
}

#[test]
fn new_session_is_a_row() {
    let now = Instant::now();
    let mut app = three(now);
    press(&mut app, KeyCode::Left, now);
    press(&mut app, KeyCode::Down, now);
    press(&mut app, KeyCode::Down, now);
    let opened = press(&mut app, KeyCode::Enter, now);
    assert_eq!(opened, Some(Action::Open(Target::New)));
}

#[test]
fn x_on_a_row_stops_that_lane() {
    let now = Instant::now();
    let mut app = three(now);
    press(&mut app, KeyCode::Left, now);
    press(&mut app, KeyCode::Up, now);
    assert_eq!(
        press(&mut app, KeyCode::Char('x'), now),
        Some(Action::Halt("B".into()))
    );
    press(&mut app, KeyCode::Down, now);
    assert_eq!(
        press(&mut app, KeyCode::Char('x'), now),
        None,
        "C has no task"
    );
    assert_eq!(app.views.shown, "A", "nothing was switched to");
}

#[test]
fn esc_or_left_closes_the_list() {
    let now = Instant::now();
    for close in [KeyCode::Esc, KeyCode::Left] {
        let mut app = three(now);
        press(&mut app, KeyCode::Left, now);
        press(&mut app, close, now);
        let screen = Term::new(&app).draw(&mut app, now);
        assert!(!screen.contains("Sessions in this project"), "{screen}");
        assert!(screen.contains("❯ Type a task"), "{screen}");
    }
}

#[test]
fn a_finished_hidden_session_is_marked_unseen_in_the_list() {
    let now = Instant::now();
    let mut app = three(now);
    app.show("B");
    app.show("A");
    let ended = Event::TaskEnded {
        status: "achieved".into(),
        millis: 188_000,
        turns: 3,
        cost: 0.0,
    };
    app.event(from("B", ended), now);
    press(&mut app, KeyCode::Left, now);
    let screen = Term::new(&app).draw(&mut app, now);
    let row = screen
        .lines()
        .find(|row| row.starts_with("  ✓ fix the flaky"));
    assert!(row.is_some_and(|row| row.contains("unseen")), "{screen}");
}

#[test]
fn opening_a_tree_row_returns_the_keys_to_input() {
    let now = Instant::now();
    for open in [KeyCode::Enter, KeyCode::Right] {
        let mut app = three(now);
        let scout = Event::Started {
            id: "s1".into(),
            name: "scout".into(),
        };
        app.event(from("A", scout), now);
        app.event(
            from("s1", Event::Note("scout's own transcript".into())),
            now,
        );
        let mut term = Term::new(&app);
        term.draw(&mut app, now);
        // ↓ into the tree on A, ↓ to its agent.
        press(&mut app, KeyCode::Down, now);
        press(&mut app, KeyCode::Down, now);
        press(&mut app, open, now);
        let screen = term.draw(&mut app, now);
        assert!(screen.contains("scout's own transcript"), "{screen}");
        assert!(
            !screen.contains("↑↓ select"),
            "focus stayed in the tree:\n{screen}"
        );
        // Left now opens the project list, rather than silently leaving tree focus.
        press(&mut app, KeyCode::Esc, now);
        assert_eq!(press(&mut app, KeyCode::Left, now), Some(Action::Sessions));
    }
}

#[test]
fn an_empty_session_has_a_clear_name_and_current_marker() {
    let now = Instant::now();
    let mut app = App::new(80, 30, vec![]);
    app.event(
        Event::LaneStarted {
            id: "empty".into(),
            name: String::new(),
        },
        now,
    );
    app.event(
        Event::LaneStarted {
            id: "saved".into(),
            name: "previous task".into(),
        },
        now,
    );
    press(&mut app, KeyCode::Left, now);
    let screen = Term::new(&app).draw(&mut app, now);
    let current = screen.lines().find(|line| line.contains("● New session"));
    assert!(
        current.is_some_and(|line| line.contains("current")),
        "{screen}"
    );
    assert!(!screen.contains("main"), "{screen}");
}

#[test]
fn a_title_made_in_the_background_renames_the_session_where_it_is_listed() {
    let now = Instant::now();
    let mut app = three(now);
    let title = crate::meta::Update {
        title: Some("Fix the flaky jobs test".into()),
    };
    app.event(from("B", Event::Updated(title)), now);
    let mut term = Term::new(&app);
    term.draw(&mut app, now);
    press(&mut app, KeyCode::Left, now);
    let screen = term.draw(&mut app, now);
    assert!(
        screen.contains("* Fix the flaky jobs test  running"),
        "{screen}"
    );
    assert!(!screen.contains("fix the flaky jobs test"), "{screen}");
}

#[test]
fn ctrl_c_in_the_list_closes_it_and_nothing_else() {
    let now = Instant::now();
    let mut app = three(now);
    press(&mut app, KeyCode::Left, now);
    assert_eq!(app.event(crate::tui::test_support::ctrl('c'), now), None);
    let screen = Term::new(&app).draw(&mut app, now);
    assert!(!screen.contains("Sessions in this project"), "{screen}");
    assert!(!screen.contains("again to quit"), "{screen}");
}
