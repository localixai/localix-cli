use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use serde_json::json;

use super::*;
use crate::tui::event::Event;
use crate::tui::style;
use crate::tui::test_support::{live_area as shown, press, typed};

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
fn idle_shows_the_current_mode_and_cycle_hint() {
    let app = app();
    let (lines, cursor) = render(&app, Instant::now());
    assert_eq!(
        lines.iter().map(style::plain).collect::<Vec<_>>(),
        [
            "❯ Type a task, / for commands",
            "  default mode · shift+tab to cycle"
        ]
    );
    assert_eq!(cursor, Some(Cursor { row: 0, column: 2 }));
}

#[test]
fn mode_stays_visible_while_typing_and_updates_for_every_mode() {
    let now = Instant::now();
    let mut app = app();
    app.event(Event::TaskStarted("work".into()), now);
    typed(&mut app, "follow up", now);
    for mode in crate::mode::Mode::ALL {
        app.event(Event::Mode(mode.name().into()), now);
        let screen = shown(&app, now);
        assert!(
            screen.contains(&format!("{} mode · shift+tab to cycle", mode.name())),
            "{screen}"
        );
        assert!(screen.contains("❯ follow up"), "{screen}");
    }
}

#[test]
fn working_shows_the_call_its_steps_the_time_and_the_queue() {
    let start = Instant::now();
    let mut app = app();
    app.event(Event::TaskStarted("check the shop".into()), start);
    app.event(
        Event::Call {
            name: "window".into(),
            args: json!({"action": "interact", "tab": "w1", "goal": "Buy one"}),
        },
        start,
    );
    app.event(Event::Step("click \"Buy\" [e12]".into()), start);
    // Tab while it works keeps the entry for after the task; the session says the mode.
    typed(&mut app, "then update the changelog", start);
    press(&mut app, KeyCode::Tab, start);
    app.event(Event::Mode("plan".into()), start);
    insta::assert_snapshot!(shown(&app, start + Duration::from_millis(12_300)), @r#"
    ⠸ Working in w1: Buy one
      └ click "Buy" [e12]

    ⠸ Working (12s · esc to interrupt)
      ⏎ then update the changelog

    ❯ Type a task, / for commands
      plan mode · shift+tab to cycle
    "#);
}

#[test]
fn slash_lists_the_commands_under_the_prompt() {
    let now = Instant::now();
    let mut app = app();
    typed(&mut app, "/", now);
    insta::assert_snapshot!(shown(&app, now), @r"
    ❯ /
      /mode       show or switch the mode
      /model      pick the model
      default mode · shift+tab to cycle
    ");
}

#[test]
fn mode_breadcrumb_and_suggestions_fit_a_narrow_terminal() {
    let now = Instant::now();
    let mut app = app();
    app.width = 20;
    typed(&mut app, "/mode ", now);
    let (lines, _) = render(&app, now);
    assert!(shown(&app, now).contains("mode  ›"));
    assert!(shown(&app, now).contains("autonomous"));
    assert!(
        lines
            .iter()
            .all(|line| crate::tui::wrap::line_width(line) <= 20)
    );
}

#[test]
fn a_long_command_in_an_approval_wraps_under_its_indent() {
    // The question takes the composer's place, and the call's and the task's rows wait.
    let now = Instant::now();
    let mut app = App::new(50, 20, vec![]);
    app.event(Event::TaskStarted("x".into()), now);
    let (reply, _answer) = sync_channel(1);
    let summary = "cd /Users/me/site && wc -l index.html v2.html style.css && grep -n -i 'viewport' index.html";
    app.event(
        Event::Approve {
            tool: "bash".into(),
            summary: summary.into(),
            reply,
        },
        now,
    );
    let (lines, _) = render(&app, now);
    assert!(
        lines
            .iter()
            .all(|line| crate::tui::wrap::line_width(line) <= 50)
    );
    insta::assert_snapshot!(shown(&app, now), @"
    • Run this command?
      cd /Users/me/site && wc -l index.html v2.html
      style.css && grep -n -i 'viewport' index.html

    ❯ 1. Yes
      2. Yes, and don't ask again for bash
      3. No, and tell llx what to do differently

      ↑↓ to choose · enter to confirm · esc to say no
    ");
}

#[test]
fn the_quit_hint_goes_once_its_window_has_passed() {
    let now = Instant::now();
    let mut app = app();
    app.event(crate::tui::test_support::ctrl('c'), now);
    assert!(shown(&app, now).contains("ctrl-c again to quit"));
    let later = shown(&app, now + Duration::from_secs(3));
    assert!(!later.contains("ctrl-c again to quit"), "{later}");
}

#[test]
fn what_waits_reads_as_the_transcript_shows_it() {
    let now = Instant::now();
    let mut app = app();
    app.event(Event::TaskStarted("x".into()), now);
    app.event(
        Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::BackTab,
            crossterm::event::KeyModifiers::SHIFT,
        )),
        now,
    );
    app.event(
        Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Up,
            crossterm::event::KeyModifiers::SHIFT,
        )),
        now,
    );
    app.lane_mut()
        .background
        .push_back("<monitor job=\"m\">\nx\n</monitor>".into());
    let screen = shown(&app, now);
    assert!(!screen.contains('\u{1b}'), "{screen}");
    insta::assert_snapshot!(screen, @"
    ⠋ Working (0s · esc to interrupt)
      ⏎ shift+tab · next mode
      ⏎ more reasoning
      ≋ m · x

    ❯ Type a task, / for commands
      default mode · shift+tab to cycle
    ");
}

#[test]
fn esc_closes_the_commands_slash_offers_and_keeps_other_text() {
    let now = Instant::now();
    let mut app = app();
    typed(&mut app, "/mo", now);
    press(&mut app, KeyCode::Esc, now);
    assert_eq!(
        shown(&app, now),
        "❯ Type a task, / for commands\n  default mode · shift+tab to cycle"
    );
    // An `@` file list is part of the words: Esc keeps them.
    app.files = crate::tui::mention::Files::with(vec!["src/main.rs".into()]);
    typed(&mut app, "see @sr", now);
    assert!(shown(&app, now).contains("src/main.rs"));
    press(&mut app, KeyCode::Esc, now);
    assert!(shown(&app, now).starts_with("❯ see @sr"));
}
