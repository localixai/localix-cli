use contracts::person::Choice;

use super::*;
use crate::testing::Typed;

fn database() -> Question {
    Question {
        id: "db_choice".into(),
        text: "Which database should the cache use?".into(),
        choices: vec![
            Choice {
                label: "Postgres (Recommended)".into(),
                description: "already in the stack; one more table".into(),
            },
            Choice {
                label: "SQLite".into(),
                description: String::new(),
            },
        ],
    }
}

fn open_question() -> Question {
    Question {
        id: "deadline".into(),
        text: "When is it due?".into(),
        choices: vec![],
    }
}

fn person<'a>(
    typed: &'a Typed,
    mode: &'a Cell<Mode>,
    stopped: &'a dyn Fn() -> bool,
) -> TerminalPerson<'a, &'a Typed> {
    TerminalPerson {
        prompter: typed,
        mode,
        stopped,
        waited: &|_| {},
    }
}

#[test]
fn options_are_numbered_and_control_characters_escaped() {
    let text = shown(&database(), 0, 2);
    assert!(
        text.contains("? db_choice (1 of 2)  Which database should the cache use?"),
        "{text}"
    );
    assert!(text.contains("    1. Postgres (Recommended)  already in the stack; one more table\n"));
    assert!(text.contains("    2. SQLite\n"));
    assert!(text.contains("Type a number, a number and a note, or your own answer; Enter skips."));
    assert!(text.ends_with("  > "));
    let alone = shown(&open_question(), 0, 1);
    assert!(alone.contains("? deadline  When is it due?"), "{alone}");
    assert!(alone.contains("Type your answer; Enter skips."));
    let hostile = Question {
        text: "ok?\r\u{1b}[2Kdelete it".into(),
        ..open_question()
    };
    let text = shown(&hostile, 0, 1);
    assert!(!text.contains('\r') && !text.contains('\u{1b}'), "{text}");
}

#[test]
fn a_line_is_a_choice_a_choice_and_a_note_or_the_persons_own_words() {
    let picked = |choice, note: Option<&str>| Answer {
        choice: Some(choice),
        note: note.map(str::to_string),
    };
    assert_eq!(answer("2\n", 2), picked(1, None));
    assert_eq!(
        answer("2, keep it local", 2),
        picked(1, Some("keep it local"))
    );
    assert_eq!(answer("1: fine", 2), picked(0, Some("fine")));
    let own = |text: &str| Answer {
        choice: None,
        note: Some(text.into()),
    };
    assert_eq!(answer("9", 2), own("9"));
    assert_eq!(answer("Friday", 0), own("Friday"));
    assert_eq!(answer("1", 0), own("1"));
    assert_eq!(answer("  \n", 2), Answer::default());
}

#[test]
fn each_question_is_asked_in_turn() {
    let typed = Typed::new(&["1\n", "Friday\n"]);
    let mode = Cell::new(Mode::Default);
    let reply = person(&typed, &mode, &|| false).ask(&[database(), open_question()]);
    let Reply::Answers(answers) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(answers[0].choice, Some(0));
    assert_eq!(answers[1].note.as_deref(), Some("Friday"));
    assert_eq!(typed.asked.borrow().len(), 2);
    assert!(typed.asked.borrow()[1].contains("(2 of 2)"));
}

#[test]
fn in_autonomous_mode_nobody_is_asked() {
    let typed = Typed::new(&["1\n"]);
    let mode = Cell::new(Mode::Autonomous);
    let reply = person(&typed, &mode, &|| false).ask(&[database()]);
    assert_eq!(reply, Reply::Nobody(AUTONOMOUS.into()));
    assert!(typed.asked.borrow().is_empty());
}

#[test]
fn a_stop_before_or_while_asking_is_the_stop() {
    let typed = Typed::new(&["1\n"]);
    let mode = Cell::new(Mode::Default);
    assert_eq!(
        person(&typed, &mode, &|| true).ask(&[database()]),
        Reply::Stopped
    );
    assert!(typed.asked.borrow().is_empty());
    let stopped = Cell::new(false);
    let stop_after_one = || {
        let was = stopped.get();
        stopped.set(true);
        was
    };
    assert_eq!(
        person(&typed, &mode, &stop_after_one).ask(&[database()]),
        Reply::Stopped,
        "stopped while the question was open"
    );
}

#[test]
fn a_terminal_that_gives_nothing_is_nobody() {
    let typed = Typed::new(&[]);
    let mode = Cell::new(Mode::Plan);
    let reply = person(&typed, &mode, &|| false).ask(&[database()]);
    assert_eq!(reply, Reply::Nobody(NO_ANSWER.into()));
}

#[test]
fn the_time_the_person_takes_is_told() {
    let typed = Typed::new(&["\n"]);
    let mode = Cell::new(Mode::Bypass);
    let told = Cell::new(0);
    let waited = |_: Duration| told.set(told.get() + 1);
    let person = TerminalPerson {
        prompter: &typed,
        mode: &mode,
        stopped: &|| false,
        waited: &waited,
    };
    assert_eq!(
        person.ask(&[open_question()]),
        Reply::Answers(vec![Answer::default()])
    );
    assert_eq!(told.get(), 1);
}
