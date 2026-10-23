use std::sync::atomic::{AtomicUsize, Ordering};

use protocol::Answer;

use super::*;
use crate::testing::answering;

fn question(id: &str) -> Question {
    Question {
        id: id.into(),
        text: format!("{id}?"),
        choices: vec![],
        multiple: false,
    }
}

/// An interface whose person answers each question with its place, `at of`; what it was asked.
fn answering_places() -> (Ui, std::thread::JoinHandle<Vec<String>>) {
    let (ui, person) = answering(|asked| match asked {
        protocol::Asked::Question { at, of, .. } => {
            let note = Some(format!("{at} of {of}"));
            let answer = Answer {
                choices: vec![],
                note,
            };
            Some(protocol::Reply::Question(Some(answer)))
        }
        _ => None,
    });
    let asked = std::thread::Builder::new().name("asked".into());
    let ids = asked.spawn(move || {
        let asked = person.join().unwrap();
        let ids = asked.into_iter().filter_map(|(_, asked)| match asked {
            protocol::Asked::Question { question, .. } => Some(question.id),
            _ => None,
        });
        ids.collect()
    });
    (ui, ids.unwrap())
}

#[test]
fn each_question_is_asked_in_turn_and_the_time_taken_is_told() {
    let (ui, person) = answering_places();
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
        let (ui, person) = answering_places();
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
