use std::sync::mpsc::sync_channel;
use std::time::Instant;

use crossterm::event::KeyCode;
use protocol::Choice as Offered;

use super::*;
use crate::tui::app::App;
use crate::tui::event::Event;
use crate::tui::test_support::{key, live_area, typed};

fn asked(choices: &[&str], multiple: bool) -> Question {
    Question {
        id: "q".into(),
        text: "Which checks?".into(),
        choices: choices
            .iter()
            .map(|label| Offered {
                label: label.to_string(),
                description: String::new(),
            })
            .collect(),
        multiple,
    }
}

fn press(app: &mut App, codes: &[KeyCode], now: Instant) {
    for code in codes {
        app.event(key(*code), now);
    }
}

#[test]
fn a_question_with_several_answers_is_checked_then_sent() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let (reply, answer) = sync_channel(1);
    let question = asked(&["Tests", "Lint"], true);
    app.event(
        Event::Ask {
            question,
            at: 0,
            of: 2,
            reply,
        },
        now,
    );
    press(
        &mut app,
        &[KeyCode::Char(' '), KeyCode::Down, KeyCode::Char(' ')],
        now,
    );
    insta::assert_snapshot!(live_area(&app, now), @"
    • Which checks? 1 of 2

      1. [x] Tests
    ❯ 2. [x] Lint
      3. Something else: type it

      ↑↓ to choose · space to check · enter to confirm · esc to skip
    ");
    press(&mut app, &[KeyCode::Enter], now);
    let sent = answer.try_recv().unwrap().unwrap();
    assert_eq!(sent.choices, [0, 1]);
}

#[test]
fn an_open_question_is_a_line_to_type_at_once() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let (reply, answer) = sync_channel(1);
    app.event(
        Event::Ask {
            question: asked(&[], false),
            at: 0,
            of: 1,
            reply,
        },
        now,
    );
    typed(&mut app, "all of them", now);
    press(&mut app, &[KeyCode::Enter], now);
    let sent = answer.try_recv().unwrap().unwrap();
    assert_eq!(sent.note.as_deref(), Some("all of them"));
}

#[test]
fn the_plan_handoff_starts_on_the_option_marked() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let (reply, picked) = sync_channel(1);
    let options = vec![
        ("Yes, in default mode".to_string(), String::new()),
        ("No, stay in plan mode".to_string(), String::new()),
    ];
    let title = "Implement this plan?".to_string();
    app.event(
        Event::Choose {
            title,
            options,
            marked: Some(1),
            reply,
        },
        now,
    );
    insta::assert_snapshot!(live_area(&app, now), @"
    • Implement this plan?

      1. Yes, in default mode
    ❯ 2. No, stay in plan mode

      ↑↓ to choose · enter to confirm · esc to cancel
    ");
    press(&mut app, &[KeyCode::Enter], now);
    assert_eq!(picked.try_recv(), Ok(Some(1)));
}

#[test]
fn a_hidden_line_never_shows_the_secret() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let (reply, line) = sync_channel(1);
    let prompt = "Paste the key:".to_string();
    app.event(
        Event::Line {
            prompt,
            hidden: true,
            reply,
        },
        now,
    );
    typed(&mut app, "sk-secret", now);
    let screen = live_area(&app, now);
    assert!(
        !screen.contains("sk-secret") && screen.contains("•••••••••"),
        "{screen}"
    );
    press(&mut app, &[KeyCode::Enter], now);
    assert_eq!(line.try_recv(), Ok(Some("sk-secret".to_string())));
}

#[test]
fn a_dismissed_question_closes_unanswered_and_the_prompt_comes_back() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let (reply, answer) = sync_channel(1);
    app.event(
        Event::Ask {
            question: asked(&["Tests"], false),
            at: 0,
            of: 1,
            reply,
        },
        now,
    );
    app.event(Event::Dismiss, now);
    assert_eq!(answer.try_recv(), Ok(None));
    assert_eq!(
        live_area(&app, now),
        "❯ Type a task, / for commands\n  default mode · shift+tab to cycle"
    );
}

#[test]
fn each_pick_sends_what_it_means() {
    let (reply, approval) = sync_channel(1);
    assert_eq!(answer(Open::Approval(reply), Picked::Option(1)), None);
    assert_eq!(approval.try_recv(), Ok(Approval::Always));
    let (reply, approval) = sync_channel(1);
    let said = answer(Open::Approval(reply), Picked::Typed(2, String::new()));
    assert_eq!(
        (said.as_deref(), approval.try_recv()),
        (Some(""), Ok(Approval::No(None)))
    );
    let (reply, asked) = sync_channel(1);
    answer(Open::Question(reply), Picked::Typed(0, String::new()));
    assert_eq!(asked.try_recv(), Ok(Some(Answer::default())));
    let (reply, asked) = sync_channel(1);
    answer(Open::Question(reply), Picked::Cancelled);
    assert_eq!(asked.try_recv(), Ok(None));
    let (reply, chosen) = sync_channel(1);
    answer(Open::Choice(reply), Picked::Cancelled);
    assert_eq!(chosen.try_recv(), Ok(None));
}
