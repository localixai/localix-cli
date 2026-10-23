use std::time::Instant;

use crossterm::event::KeyCode;
use protocol::{Asked, Choice as Offered, ToClient};

use super::*;
use crate::tui::app::App;
use crate::tui::test_support::{key, live_area, replies, typed};

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
        app.input(key(*code), now);
    }
}

#[test]
fn a_question_with_several_answers_is_checked_then_sent() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let question = asked(&["Tests", "Lint"], true);
    let ask = Asked::Question {
        question,
        at: 0,
        of: 2,
    };
    app.asked(1, "", ask, now);
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
    let sent = answered(&mut app);
    assert_eq!(sent.choices, [0, 1]);
}

#[test]
fn an_open_question_is_a_line_to_type_at_once() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let ask = Asked::Question {
        question: asked(&[], false),
        at: 0,
        of: 1,
    };
    app.asked(1, "", ask, now);
    typed(&mut app, "all of them", now);
    press(&mut app, &[KeyCode::Enter], now);
    let sent = answered(&mut app);
    assert_eq!(sent.note.as_deref(), Some("all of them"));
}

#[test]
fn the_plan_handoff_starts_on_the_option_marked() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let options = vec![
        ("Yes, in default mode".to_string(), String::new()),
        ("No, stay in plan mode".to_string(), String::new()),
    ];
    let title = "Implement this plan?".to_string();
    let choose = Asked::Choose {
        title,
        options,
        marked: Some(1),
    };
    app.asked(1, "", choose, now);
    insta::assert_snapshot!(live_area(&app, now), @"
    • Implement this plan?

      1. Yes, in default mode
    ❯ 2. No, stay in plan mode

      ↑↓ to choose · enter to confirm · esc to cancel
    ");
    press(&mut app, &[KeyCode::Enter], now);
    assert_eq!(replies(&mut app), [(1, Reply::Choice(Some(1)))]);
}

#[test]
fn a_hidden_line_never_shows_the_secret() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let prompt = "Paste the key:".to_string();
    app.asked(
        1,
        "",
        Asked::Line {
            prompt,
            hidden: true,
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
    let line = Reply::Line(Some("sk-secret".to_string()));
    assert_eq!(replies(&mut app), [(1, line)]);
}

#[test]
fn a_withdrawn_question_closes_unanswered_and_the_prompt_comes_back() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let ask = Asked::Question {
        question: asked(&["Tests"], false),
        at: 0,
        of: 1,
    };
    app.asked(1, "", ask, now);
    app.frame(ToClient::Resolved { id: 1 }, now);
    assert_eq!(replies(&mut app), [], "nobody waits for a reply");
    assert_eq!(
        live_area(&app, now),
        "❯ Type a task, / for commands\n  default mode · shift+tab to cycle"
    );
}

#[test]
fn each_pick_sends_what_it_means() {
    let approval = answer(&Open::Approval(1), Picked::Option(1));
    assert_eq!(
        approval,
        (Some((1, Reply::Approval(Approval::Always))), None)
    );
    let said = answer(&Open::Approval(2), Picked::Typed(2, String::new()));
    let no = Reply::Approval(Approval::No(None));
    assert_eq!(said, (Some((2, no)), Some(String::new())));
    let typed = answer(&Open::Question(3), Picked::Typed(0, String::new()));
    let empty = Reply::Question(Some(Answer::default()));
    assert_eq!(typed, (Some((3, empty)), None));
    let skipped = answer(&Open::Question(4), Picked::Cancelled);
    assert_eq!(skipped, (Some((4, Reply::Question(None))), None));
    let cancelled = answer(&Open::Choice(5), Picked::Cancelled);
    assert_eq!(cancelled, (Some((5, Reply::Choice(None))), None));
    assert_eq!(answer(&Open::Quit, Picked::Option(0)), (None, None));
}

/// The one answer the person gave to question 1.
fn answered(app: &mut App) -> Answer {
    match &replies(app)[..] {
        [(1, Reply::Question(Some(answer)))] => answer.clone(),
        other => panic!("not an answer: {other:?}"),
    }
}
