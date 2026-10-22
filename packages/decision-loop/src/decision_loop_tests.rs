use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::Value;

use super::*;
use contracts::hands::{Hands, Outcome, Task};
use contracts::model::{Answer, DecideError, Decision, Question};
use contracts::view::{Action, Element, Observation, View, ViewResult};

/// A quiz of `questions` views behind a Next button; with `stuck`, Next does nothing.
struct Quiz {
    question: Mutex<usize>,
    stuck: bool,
}

impl View for Quiz {
    fn observe(&self) -> ViewResult<Observation> {
        let question = *self.question.lock().unwrap();
        Ok(Observation {
            outline: String::new(),
            epoch: question.to_string(),
            location: "http://quiz".into(),
            title: "Quiz".into(),
            text: format!("Question {question} of 25"),
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
        })
    }
    fn act(&self, _: &str, _: &Action) -> ViewResult<()> {
        if !self.stuck {
            *self.question.lock().unwrap() += 1;
        }
        Ok(())
    }
}

/// Clicks Next until the view says the quiz is over, then DONE.
struct Taker;

fn sure(choice: &str) -> Answer {
    Answer::Choice {
        choice: choice.into(),
        probabilities: [(choice.to_string(), 1.0)].into(),
        confidence: 1.0,
    }
}

impl Decider for Taker {
    fn decide(
        &self,
        state: &Value,
        _: &BTreeMap<String, Question>,
    ) -> Result<Decision, DecideError> {
        let over = state["view"]["text"] == "Question 26 of 25";
        let answers = if over {
            [("operation".to_string(), sure("DONE"))].into()
        } else {
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

fn run(stuck: bool) -> GoalOutcome {
    let quiz = Quiz {
        question: Mutex::new(1),
        stuck,
    };
    DecisionLoop {
        decider: &Taker,
        on_step: &|_| {},
        max_actions: 100,
    }
    .run(&quiz, "Answer all 25 questions", &[])
}

#[test]
fn a_goal_longer_than_one_run_is_carried_to_done() {
    let outcome = run(false);
    assert_eq!(outcome.stop, Stop::Done { probability: 1.0 });
    let clicks = outcome
        .steps
        .iter()
        .filter(|step| step.operation == Operation::Click)
        .count();
    assert_eq!(clicks, 25, "past the 20 actions one run takes");
}

#[test]
fn runs_that_change_nothing_end_the_loop() {
    let outcome = run(true);
    assert_eq!(outcome.stop, Stop::Repeated);
    assert!(
        outcome.steps.len() <= 2 * MAX_STALLS,
        "{}",
        outcome.steps.len()
    );
}

#[test]
fn the_hands_report_each_step_and_how_the_task_ended() {
    let quiz = Quiz {
        question: Mutex::new(24),
        stuck: false,
    };
    let hands = DecisionLoop {
        decider: &Taker,
        on_step: &|_| {},
        max_actions: 10,
    };
    let report = hands.carry_out(
        &quiz,
        Task {
            goal: "Finish the quiz",
            values: &[],
        },
    );
    assert_eq!(report.outcome, Outcome::Done { probability: 1.0 });
    assert_eq!(report.steps.len(), 3, "{:?}", report.steps);
    assert!(report.steps[0].starts_with("CLICK"), "{:?}", report.steps);
}
