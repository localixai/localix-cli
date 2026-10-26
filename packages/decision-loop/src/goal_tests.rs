use super::*;

/// The app's pause, as the binary gives it: a test's view has no change event either.
#[expect(
    clippy::disallowed_methods,
    reason = "a poll, as the app's: a view has no change event"
)]
fn pause(time: Duration) {
    std::thread::sleep(time);
}

#[test]
fn a_loop_is_a_state_seen_again_not_a_repeated_action() {
    // Open search, close it, open it again: the third visit to the same state is a loop.
    let (closed, open) = (1, 2);
    assert!(!cycling(&[closed, open], closed));
    assert!(cycling(&[closed, open, closed, open], closed));
    // "Scroll right" five times, each showing new dates: never a loop.
    assert!(!cycling(&[10, 11, 12, 13], 14));
}

mod covered {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use serde_json::Value;

    use super::*;
    use contracts::model::DecideError;
    use contracts::model::{Answer, Decision, Question};
    use contracts::view::{Action, Element, View, ViewResult};

    /// A view whose first click is refused because a banner came over it after the snapshot.
    struct Banner {
        acts: Mutex<usize>,
    }

    fn view(question: usize) -> Observation {
        Observation {
            outline: String::new(),
            epoch: "1".into(),
            location: "http://x".into(),
            title: "Quiz".into(),
            text: format!("Question {question}"),
            scroll: 0,
            elements: vec![Element {
                reference: "e1".into(),
                role: "button".into(),
                name: "Next".into(),
                value: None,
                states: vec![],
                editable: false,
                dialog: false,
                options: vec![],
            }],
            omitted: 0,
            busy: false,
        }
    }

    impl View for Banner {
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn observe(&self) -> ViewResult<Observation> {
            // The click that went through moved on to the next question.
            let acts = *self.acts.lock().unwrap();
            Ok(view(if acts >= 2 { 2 } else { 1 }))
        }
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn act(&self, _: &str, _: &Action) -> ViewResult<()> {
            let mut acts = self.acts.lock().unwrap();
            *acts += 1;
            if *acts == 1 {
                return Err(ViewError::Refused(
                    "covered: iframe \"Consent\" is on top".into(),
                ));
            }
            Ok(())
        }
    }

    /// Clicks Next until it has clicked once, then says DONE.
    struct Model {
        clicked: Mutex<usize>,
    }

    fn sure(choice: &str) -> Answer {
        Answer::Choice {
            choice: choice.into(),
            probabilities: [(choice.to_string(), 1.0)].into(),
            confidence: 1.0,
        }
    }

    impl Decider for Model {
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn decide(
            &self,
            state: &Value,
            _: &BTreeMap<String, Question>,
        ) -> Result<Decision, DecideError> {
            let mut clicked = self.clicked.lock().unwrap();
            let done = state["recent_actions"]
                .as_array()
                .is_some_and(|steps| steps.iter().any(|s| !s.to_string().contains("failed")));
            let answers = if done {
                [("operation".to_string(), sure("DONE"))].into()
            } else {
                *clicked += 1;
                [
                    ("operation".to_string(), sure("CLICK")),
                    ("click_target".to_string(), sure("e1")),
                ]
                .into()
            };
            Ok(Decision {
                answers,
                ..Decision::default()
            })
        }
    }

    #[test]
    fn a_click_under_a_late_banner_is_decided_again_not_the_end() {
        let driver = Banner {
            acts: Mutex::new(0),
        };
        let model = Model {
            clicked: Mutex::new(0),
        };
        let outcome = GoalRun {
            view: &driver,
            decider: &model,
            on_step: &|_| {},
            pause: &pause,
            max_actions: 5,
        }
        .run("Click Next", &[]);
        assert_eq!(outcome.stop, Stop::Done { probability: 1.0 });
        assert!(
            outcome.steps[0]
                .failed
                .as_deref()
                .unwrap()
                .starts_with("covered")
        );
        assert_eq!(
            *model.clicked.lock().unwrap(),
            2,
            "clicked again after the refusal"
        );
    }

    /// Says DONE, unsure, every time it is asked.
    struct Unsure {
        asked: Mutex<usize>,
    }

    impl Decider for Unsure {
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn decide(
            &self,
            _: &Value,
            _: &BTreeMap<String, Question>,
        ) -> Result<Decision, DecideError> {
            *self.asked.lock().unwrap() += 1;
            let done = Answer::Choice {
                choice: "DONE".into(),
                probabilities: [("DONE".to_string(), 0.55), ("CLICK".to_string(), 0.45)].into(),
                confidence: 0.55,
            };
            Ok(Decision {
                answers: [("operation".to_string(), done)].into(),
                ..Decision::default()
            })
        }
    }

    #[test]
    fn an_unsure_done_on_a_view_that_stays_put_is_not_asked_again() {
        let driver = Banner {
            acts: Mutex::new(0),
        };
        let model = Unsure {
            asked: Mutex::new(0),
        };
        let outcome = GoalRun {
            view: &driver,
            decider: &model,
            on_step: &|_| {},
            pause: &pause,
            max_actions: 5,
        }
        .run("Answer three questions", &[]);
        assert_eq!(outcome.stop, Stop::Done { probability: 0.55 });
        assert_eq!(
            *model.asked.lock().unwrap(),
            1,
            "asking again invites a step too many"
        );
    }
}
