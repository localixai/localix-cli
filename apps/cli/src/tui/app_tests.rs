use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use contracts::tool::ToolOutput;
use crossterm::event::KeyCode;
use serde_json::json;

use super::*;
use crate::tui::style;
use crate::tui::test_support::{ctrl, history, key, next_entry, render, typed};

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

/// When a question was opened that takes keys at `now`: an approval ignores them at first.
fn opened(now: Instant) -> Instant {
    now.checked_sub(Duration::from_millis(300)).unwrap_or(now)
}

#[test]
fn an_entry_goes_to_the_session_when_it_waits_steers_it_while_it_works_or_waits_with_tab() {
    let now = Instant::now();
    let mut app = app();
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    typed(&mut app, "first", now);
    app.event(key(KeyCode::Enter), now);
    assert_eq!(entries.try_recv(), Ok(Some("first".into())));
    app.event(Event::TaskStarted("first".into()), now);
    typed(&mut app, "now", now);
    app.event(key(KeyCode::Enter), now);
    assert_eq!(app.lane().steered, [Typed::from("now")]);
    typed(&mut app, "/status", now);
    app.event(key(KeyCode::Enter), now);
    typed(&mut app, "second", now);
    app.event(key(KeyCode::Tab), now);
    assert_eq!(
        app.lane().queued,
        [Typed::from("/status"), Typed::from("second")]
    );
    let (reply, taken) = channel();
    app.event(Event::TakeSteered { reply, peek: false }, now);
    assert_eq!(taken.try_recv(), Ok(vec!["now".to_string()]));
    assert!(app.lane().steered.is_empty());
    app.event(
        Event::TaskEnded {
            status: "achieved".into(),
            millis: 1,
            turns: 1,
            cost: 0.0,
        },
        now,
    );
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    assert_eq!(entries.try_recv(), Ok(Some("/status".into())));
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    assert_eq!(entries.try_recv(), Ok(Some("second".into())));
    assert!(app.lane().queued.is_empty());
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
    assert!(!app.shown().running.is_empty());
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
    assert!(app.busy().is_none() && app.shown().running.is_empty());
}

#[test]
fn ctrl_c_stops_a_task_clears_the_line_and_quits_only_when_pressed_twice() {
    let now = Instant::now();
    let mut app = app();
    app.event(Event::TaskStarted("x".into()), now);
    assert_eq!(app.event(ctrl('c'), now), Some(Action::Stop(String::new())));
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
    // Approvals come while a task works.
    app.event(Event::TaskStarted("a task".into()), now);
    let (reply, answer) = channel();
    app.event(
        Event::Approve {
            tool: "bash".into(),
            summary: "run cargo test".into(),
            reply,
        },
        opened(now),
    );
    app.event(key(KeyCode::Char('3')), now);
    typed(&mut app, "use make", now);
    app.event(key(KeyCode::Enter), now);
    assert_eq!(answer.try_recv(), Ok(Approval::No(Some("use make".into()))));
    assert!(app.questions.is_empty());
    let (reply, answer) = channel();
    app.event(
        Event::Approve {
            tool: "bash".into(),
            summary: "rm -rf build".into(),
            reply,
        },
        opened(now),
    );
    assert_eq!(app.event(ctrl('c'), now), Some(Action::Stop(String::new())));
    assert_eq!(answer.try_recv(), Ok(Approval::No(None)));
}

#[test]
fn keys_typed_ahead_as_an_approval_opens_do_not_answer_it() {
    let now = Instant::now();
    let mut app = app();
    // Approvals come while a task works.
    app.event(Event::TaskStarted("a task".into()), now);
    let (reply, answer) = channel();
    let approve = Event::Approve {
        tool: "bash".into(),
        summary: "rm -rf build".into(),
        reply,
    };
    app.event(approve, now);
    // Typed for the composer just as the question opened: j, k, digits and Enter go nowhere.
    let soon = now + Duration::from_millis(120);
    typed(&mut app, "jk2", soon);
    app.event(key(KeyCode::Enter), soon);
    assert_eq!(answer.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty));
    assert!(!app.questions.is_empty(), "the approval is still open");
    let later = now + Duration::from_millis(300);
    app.event(key(KeyCode::Char('3')), later);
    typed(&mut app, "use make", later);
    app.event(key(KeyCode::Enter), later);
    assert_eq!(answer.try_recv(), Ok(Approval::No(Some("use make".into()))));
    let (reply, answer) = channel();
    let approve = Event::Approve {
        tool: "bash".into(),
        summary: "rm -rf build".into(),
        reply,
    };
    app.event(approve, now);
    // Ctrl-C still stops at once.
    assert_eq!(
        app.event(ctrl('c'), soon),
        Some(Action::Stop(String::new()))
    );
    assert_eq!(answer.try_recv(), Ok(Approval::No(None)));
}

#[test]
fn slash_offers_the_commands_and_enter_takes_the_marked_one() {
    let now = Instant::now();
    let mut app = app();
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    typed(&mut app, "/", now);
    insta::assert_snapshot!(render(&mut app, now), @"
    ❯ /
      /help       the commands
      /mode       show or switch the mode
      /new        start over
    ");
    typed(&mut app, "n", now);
    insta::assert_snapshot!(render(&mut app, now), @"
    ❯ /n
      /new        start over
    ");
    let opened = app.event(key(KeyCode::Enter), now);
    assert_eq!(
        opened,
        Some(Action::Open(crate::desk::Target::New)),
        "the marked one"
    );
    assert!(
        entries.try_recv().is_err(),
        "the desk opens it, not the lane"
    );
    typed(&mut app, "/m", now);
    app.event(key(KeyCode::Tab), now);
    typed(&mut app, "plan", now);
    insta::assert_snapshot!(render(&mut app, now), @"
    ❯ /mode plan
      mode  ›  current: default · ↑↓ choose · tab complete · en…
      plan         Investigate and plan: file changes are refus…
    ");
    app.event(key(KeyCode::Enter), now);
    assert_eq!(entries.try_recv(), Ok(Some("/mode plan".into())));
}

#[test]
fn command_drill_down_shows_modes_and_preserves_argument_entry() {
    let now = Instant::now();
    let mut app = app();
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    typed(&mut app, "/mo", now);
    app.event(key(KeyCode::Enter), now);
    assert_eq!(
        entries.try_recv(),
        Ok(Some("/mode".into())),
        "a part runs the marked one"
    );
    let sent = |app: &mut App, text: &str| {
        typed(app, text, now);
        app.event(key(KeyCode::Enter), now);
        next_entry(app, now)
    };
    assert_eq!(sent(&mut app, "/mode").as_deref(), Some("/mode"));
    assert_eq!(sent(&mut app, "/plan").as_deref(), Some("/plan"));
    typed(&mut app, "/mode ", now);
    insta::assert_snapshot!(render(&mut app, now), @"
    ❯ /mode
      mode  ›  current: default · ↑↓ choose · tab complete · en…
      default      Each command, file change and browser action…
      plan         Investigate and plan: file changes are refus…
      bypass       Everything runs without asking (-y); questio…
      autonomous   Everything runs without asking, and no quest…
    ");
    typed(&mut app, "aut", now);
    app.event(key(KeyCode::Tab), now);
    assert_eq!(sent(&mut app, "").as_deref(), Some("/mode autonomous"));
    assert_eq!(
        sent(&mut app, "/permissions by").as_deref(),
        Some("/permissions bypass"),
        "a part runs the mode marked"
    );
}

#[test]
fn freeform_plan_task_and_unknown_mode_are_not_replaced_by_suggestions() {
    let now = Instant::now();
    let mut app = app();
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    typed(&mut app, "/plan fix the tests", now);
    assert!(app.offers().is_empty());
    app.event(key(KeyCode::Enter), now);
    assert_eq!(entries.try_recv(), Ok(Some("/plan fix the tests".into())));
    typed(&mut app, "/mode custom", now);
    assert!(app.offers().is_empty());
    app.event(key(KeyCode::Enter), now);
    assert_eq!(app.lane().queued[0].text, "/mode custom");
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
        opened(now),
    );
    app.event(key(KeyCode::Char('3')), now);
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
fn pasted_text_is_shown_as_a_mark_but_sent_as_text() {
    let now = Instant::now();
    let mut app = app();
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    typed(&mut app, "review ", now);
    app.event(Event::Paste("first\r\nsecond".into()), now);
    assert_eq!(app.composer.text(), "review [Paste] ");
    app.event(key(KeyCode::Enter), now);
    assert_eq!(
        entries.try_recv().unwrap().unwrap().text,
        "review first\nsecond "
    );
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

#[test]
fn slash_then_arrows_then_enter_runs_the_command_chosen() {
    // The bug: "/" alone, Down to a command, Enter sent "/" and the session said it was unknown.
    let now = Instant::now();
    let mut app = app();
    let (reply, entries) = channel();
    app.event(Event::Ready(reply), now);
    typed(&mut app, "/", now);
    app.event(key(KeyCode::Down), now);
    app.event(key(KeyCode::Enter), now);
    assert_eq!(entries.try_recv(), Ok(Some("/mode".into())));
    assert!(app.composer.is_empty());
}

#[test]
fn a_refusal_is_drawn_on_the_call_refused_not_on_another_that_failed_meanwhile() {
    let now = Instant::now();
    let mut app = app();
    let (refused, other) = (json!({"command": "git log"}), json!({"command": "false"}));
    for args in [&refused, &other] {
        let (name, args) = ("bash".to_string(), args.clone());
        app.event(Event::Call { name, args }, now);
    }
    let (reply, _answer) = channel();
    app.event(
        Event::Approve {
            tool: "bash".into(),
            summary: "git log".into(),
            reply,
        },
        opened(now),
    );
    app.event(key(KeyCode::Esc), now);
    assert_eq!(app.lane().declined.as_deref(), Some(""));
    let failed = |text: &str| Err(contracts::tool::ToolError::Failed(text.into()));
    for (args, outcome) in [
        (other, failed("exit 1")),
        (
            refused,
            failed("not allowed: the person declined it; do not try the same thing another way"),
        ),
    ] {
        let name = "bash".to_string();
        app.event(
            Event::Result {
                name,
                args,
                outcome,
            },
            now,
        );
    }
    let shown = history(&mut app);
    assert!(!shown.contains("Skipped false"), "{shown}");
    assert!(shown.contains("Skipped git log"), "{shown}");
}

/// Two lanes, A (shown, started first) and B, each with its name.
fn lanes(now: Instant) -> App {
    let mut app = app();
    for (id, name) in [("A", "fix the flaky jobs test"), ("B", "add tabs")] {
        let (id, name) = (id.into(), name.into());
        app.event(Event::LaneStarted { id, name }, now);
    }
    app
}

/// `event` as lane (or agent) `id` sends it.
fn from(id: &str, event: Event) -> Event {
    Event::In(id.into(), Box::new(event))
}

/// Lane `id` asks for its next entry; where it arrives.
fn ready(app: &mut App, id: &str, now: Instant) -> std::sync::mpsc::Receiver<Option<Typed>> {
    let (reply, entries) = channel();
    app.event(from(id, Event::Ready(reply)), now);
    entries
}

#[test]
fn typing_goes_to_the_shown_lane() {
    let now = Instant::now();
    let mut app = lanes(now);
    let (a, b) = (ready(&mut app, "A", now), ready(&mut app, "B", now));
    app.show("B");
    typed(&mut app, "for b", now);
    app.event(key(KeyCode::Enter), now);
    assert_eq!(b.try_recv(), Ok(Some("for b".into())));
    assert!(a.try_recv().is_err(), "A still waits");
}

#[test]
fn steered_is_per_lane() {
    let now = Instant::now();
    let mut app = lanes(now);
    app.event(from("A", Event::TaskStarted("a's task".into())), now);
    app.event(from("B", Event::TaskStarted("b's task".into())), now);
    app.show("B");
    typed(&mut app, "steer b", now);
    app.event(key(KeyCode::Enter), now);
    let take = |app: &mut App, id: &str| {
        let (reply, taken) = channel();
        app.event(from(id, Event::TakeSteered { reply, peek: false }), now);
        taken.try_recv().unwrap()
    };
    assert!(take(&mut app, "A").is_empty());
    assert_eq!(take(&mut app, "B"), ["steer b"]);
}

#[test]
fn background_mail_goes_to_its_lane() {
    let now = Instant::now();
    let mut app = lanes(now);
    app.event(
        from("A", Event::Background("<monitor>up</monitor>".into())),
        now,
    );
    let b = ready(&mut app, "B", now);
    assert!(b.try_recv().is_err(), "B has no mail");
    let a = ready(&mut app, "A", now);
    assert_eq!(a.try_recv(), Ok(Some("<monitor>up</monitor>".into())));
}

#[test]
fn dismiss_closes_only_its_own_lanes_question() {
    let now = Instant::now();
    let mut app = lanes(now);
    let (reply, answer) = channel();
    let approve = Event::Approve {
        tool: "bash".into(),
        summary: "cargo test".into(),
        reply,
    };
    app.event(from("A", approve), opened(now));
    app.event(from("B", Event::Dismiss), now);
    assert!(answer.try_recv().is_err(), "A's question is still open");
    app.event(from("A", Event::Dismiss), now);
    assert_eq!(answer.try_recv(), Ok(Approval::No(None)));
}

#[test]
fn leave_answers_every_lane() {
    let now = Instant::now();
    let mut app = lanes(now);
    let (a, b) = (ready(&mut app, "A", now), ready(&mut app, "B", now));
    let (reply, picked) = channel();
    let choose = Event::Choose {
        title: "Implement this plan?".into(),
        options: vec![("Yes".into(), String::new())],
        marked: None,
        reply,
    };
    app.event(from("B", choose), now);
    app.leave();
    assert_eq!((a.try_recv(), b.try_recv()), (Ok(None), Ok(None)));
    assert_eq!(picked.try_recv(), Ok(None));
}

#[test]
fn esc_from_agent_view_shows_its_parent_lane() {
    let now = Instant::now();
    let mut app = lanes(now);
    let started = Event::Started {
        id: "scout".into(),
        name: "scout".into(),
    };
    app.event(from("B", started), now);
    app.show("scout");
    app.event(key(KeyCode::Esc), now);
    assert_eq!(app.views.shown, "B");
}

#[test]
fn exit_typed_in_any_lane_quits_the_interface() {
    let now = Instant::now();
    let mut app = lanes(now);
    let b = ready(&mut app, "B", now);
    app.show("B");
    typed(&mut app, "/exit", now);
    assert_eq!(app.event(key(KeyCode::Enter), now), Some(Action::Quit));
    assert!(b.try_recv().is_err(), "the lane is not told: every lane is");
}

/// An approval of `summary` from lane `id`; where its answer goes.
fn approve(
    app: &mut App,
    id: &str,
    summary: &str,
    at: Instant,
) -> std::sync::mpsc::Receiver<Approval> {
    let (reply, answer) = channel();
    let event = Event::Approve {
        tool: "bash".into(),
        summary: summary.into(),
        reply,
    };
    app.event(from(id, event), at);
    answer
}

/// A choice of llx's own from lane `id`; where its pick goes.
fn choose(app: &mut App, id: &str, now: Instant) -> std::sync::mpsc::Receiver<Option<usize>> {
    let (reply, picked) = channel();
    let event = Event::Choose {
        title: "Implement this plan?".into(),
        options: vec![("Yes".into(), String::new()), ("No".into(), String::new())],
        marked: None,
        reply,
    };
    app.event(from(id, event), now);
    picked
}

#[test]
fn hidden_lane_approval_opens_labelled() {
    let now = Instant::now();
    let mut app = lanes(now);
    app.show("B");
    let _answer = approve(&mut app, "A", "cargo test -p jobs", now);
    let shown = crate::tui::test_support::live_area(&app, now);
    assert!(
        shown.contains("session fix the flaky jobs test (in the background)"),
        "{shown}"
    );
}

#[test]
fn own_lane_approval_has_no_label() {
    let now = Instant::now();
    let mut app = lanes(now);
    let _answer = approve(&mut app, "A", "cargo test -p jobs", now);
    let shown = crate::tui::test_support::live_area(&app, now);
    assert!(shown.contains("cargo test -p jobs"), "{shown}");
    assert!(!shown.contains("session "), "{shown}");
}

#[test]
fn second_question_waits() {
    let now = Instant::now();
    let mut app = lanes(now);
    let a = approve(&mut app, "A", "first command", opened(now));
    let b = approve(&mut app, "B", "second command", opened(now));
    let shown = crate::tui::test_support::live_area(&app, now);
    assert!(shown.contains("first command"), "{shown}");
    assert!(!shown.contains("second command"), "{shown}");
    assert!(shown.contains("1 more question waiting"), "{shown}");
    app.event(key(KeyCode::Char('1')), now);
    assert_eq!(a.try_recv(), Ok(Approval::Yes));
    let later = now + Duration::from_millis(300);
    let shown = crate::tui::test_support::live_area(&app, later);
    assert!(shown.contains("second command"), "{shown}");
    assert!(!shown.contains("more question"), "{shown}");
    app.event(key(KeyCode::Esc), later);
    assert_eq!(b.try_recv(), Ok(Approval::No(None)));
    assert!(app.questions.is_empty());
}

#[test]
fn held_choice_waits_until_shown() {
    let now = Instant::now();
    let mut app = lanes(now);
    let picked = choose(&mut app, "B", now);
    assert!(app.questions.is_empty(), "held until B is shown");
    let shown = crate::tui::test_support::live_area(&app, now);
    assert!(!shown.contains("Implement this plan?"), "{shown}");
    app.show("B");
    let shown = crate::tui::test_support::live_area(&app, now);
    assert!(shown.contains("Implement this plan?"), "{shown}");
    app.event(key(KeyCode::Enter), now);
    assert_eq!(picked.try_recv(), Ok(Some(0)));
}

#[test]
fn held_choice_does_not_block_other_lanes() {
    let now = Instant::now();
    let mut app = lanes(now);
    let picked = choose(&mut app, "B", now);
    let a = approve(&mut app, "A", "cargo build", opened(now));
    app.event(key(KeyCode::Char('1')), now);
    assert_eq!(a.try_recv(), Ok(Approval::Yes));
    assert!(picked.try_recv().is_err(), "B's choice still waits for B");
}

#[test]
fn keys_within_300ms_of_a_hidden_question_are_ignored() {
    let now = Instant::now();
    let mut app = lanes(now);
    let (reply, answer) = channel();
    let question = contracts::person::Question {
        id: "file".into(),
        text: "Which file?".into(),
        choices: vec![contracts::person::Choice {
            label: "a.rs".into(),
            description: String::new(),
        }],
        multiple: false,
    };
    let ask = Event::Ask {
        question,
        at: 0,
        of: 1,
        reply,
    };
    app.event(from("B", ask), now);
    app.event(key(KeyCode::Char('1')), now + Duration::from_millis(100));
    assert!(answer.try_recv().is_err(), "typed for the composer");
    app.event(key(KeyCode::Char('1')), now + Duration::from_millis(300));
    assert!(answer.try_recv().unwrap().is_some());
}

#[test]
fn ctrl_c_in_question_stops_the_asker() {
    let now = Instant::now();
    let mut app = lanes(now);
    app.event(from("A", Event::TaskStarted("a's task".into())), now);
    app.show("B");
    let a = approve(&mut app, "A", "rm -rf build", now);
    assert_eq!(app.event(ctrl('c'), now), Some(Action::Stop("A".into())));
    assert_eq!(a.try_recv(), Ok(Approval::No(None)));
}

#[test]
fn ctrl_c_in_an_idle_lanes_choice_only_closes_it() {
    // Stopping a lane with no task quits llx, the other lanes' work with it.
    let now = Instant::now();
    let mut app = lanes(now);
    let picked = choose(&mut app, "A", now);
    assert_eq!(app.event(ctrl('c'), now), None);
    assert_eq!(picked.try_recv(), Ok(None));
}

#[test]
fn a_lanes_own_choice_after_the_person_left_gets_nothing() {
    // A hidden lane's held choice would never be shown, and the desk would wait for it.
    let now = Instant::now();
    let mut app = lanes(now);
    app.leave();
    let (a, b) = (choose(&mut app, "A", now), choose(&mut app, "B", now));
    assert_eq!((a.try_recv(), b.try_recv()), (Ok(None), Ok(None)));
    assert!(app.questions.is_empty());
}

#[test]
fn new_typed_while_a_task_works_opens_a_lane_at_once() {
    let now = Instant::now();
    let mut app = lanes(now);
    app.event(from("A", Event::TaskStarted("a's task".into())), now);
    typed(&mut app, "/new", now);
    let opened = app.event(key(KeyCode::Enter), now);
    assert_eq!(opened, Some(Action::Open(crate::desk::Target::New)));
    assert!(app.lane().queued.is_empty(), "not kept for after the task");
}

#[test]
fn the_desk_shows_a_lane_it_opened() {
    let now = Instant::now();
    let mut app = lanes(now);
    app.event(Event::Show("B".into()), now);
    assert_eq!(app.views.shown, "B");
}

#[test]
fn a_background_questions_label_goes_once_its_lane_is_shown() {
    let now = Instant::now();
    let mut app = lanes(now);
    app.show("B");
    let _answer = approve(&mut app, "A", "cargo test -p jobs", now);
    app.show("A");
    let shown = crate::tui::test_support::live_area(&app, now);
    assert!(shown.contains("cargo test -p jobs"), "{shown}");
    assert!(!shown.contains("in the background"), "{shown}");
}

#[test]
fn a_choice_after_a_dismissed_approval_takes_keys_at_once() {
    let now = Instant::now();
    let mut app = lanes(now);
    let _answer = approve(&mut app, "A", "cargo build", now);
    app.event(from("A", Event::Dismiss), now);
    let picked = choose(&mut app, "A", now);
    app.event(key(KeyCode::Enter), now + Duration::from_millis(10));
    assert_eq!(
        picked.try_recv(),
        Ok(Some(0)),
        "not swallowed by the approval's settling"
    );
}
