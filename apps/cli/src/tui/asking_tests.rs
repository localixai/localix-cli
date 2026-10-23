use std::sync::atomic::{AtomicUsize, Ordering};

use contracts::person::Answer;

use super::*;
use crate::tui::event::Event;
use crate::tui::test_support::fake_ui;

fn question(id: &str) -> Question {
    Question {
        id: id.into(),
        text: format!("{id}?"),
        choices: vec![],
        multiple: false,
    }
}

/// An interface whose person answers each question with its place, `at of`; what it was asked.
fn answering() -> (Ui, std::thread::JoinHandle<Vec<String>>) {
    let (ui, events) = fake_ui();
    let person = std::thread::spawn(move || {
        let mut asked = vec![];
        for event in events {
            if let Event::Ask {
                question,
                at,
                of,
                reply,
            } = event
            {
                asked.push(question.id);
                let note = Some(format!("{at} of {of}"));
                let _ = reply.send(Some(Answer {
                    choices: vec![],
                    note,
                }));
            }
        }
        asked
    });
    (ui, person)
}

#[test]
fn each_question_is_asked_in_turn_and_the_time_taken_is_told() {
    let (ui, person) = answering();
    let mode = ModeCell::new(Mode::Default);
    let told = AtomicUsize::new(0);
    let waited = |_: Duration| {
        told.fetch_add(1, Ordering::SeqCst);
    };
    let asking = UiPerson {
        ui,
        mode: &mode,
        stopped: &|| false,
        waited: &waited,
    };
    let reply = asking.ask(&[question("db"), question("deadline")]);
    let notes: Vec<String> = match reply {
        Reply::Answers(answers) => answers.into_iter().map(|a| a.note.unwrap()).collect(),
        other => panic!("{other:?}"),
    };
    assert_eq!(notes, vec!["0 of 2", "1 of 2"]);
    assert_eq!(told.load(Ordering::SeqCst), 1);
    drop(asking);
    assert_eq!(person.join().unwrap(), ["db", "deadline"]);
}

#[test]
fn in_autonomous_mode_or_once_stopped_nobody_is_asked() {
    for (mode, stopped, expected) in [
        (Mode::Autonomous, false, Reply::Nobody(AUTONOMOUS.into())),
        (Mode::Default, true, Reply::Stopped),
    ] {
        let (ui, person) = answering();
        let mode = ModeCell::new(mode);
        let is_stopped = move || stopped;
        let asking = UiPerson {
            ui,
            mode: &mode,
            stopped: &is_stopped,
            waited: &|_| {},
        };
        assert_eq!(asking.ask(&[question("db")]), expected);
        drop(asking);
        assert!(person.join().unwrap().is_empty());
    }
}
