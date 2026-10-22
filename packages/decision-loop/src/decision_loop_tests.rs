use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::Value;

use super::*;
use crate::toolbox::Toolbox;
use agent::model::{Answer, DecideError, Decision, Question};
use agent::tool::{Action, Element, Observation, Tool, ToolResult};

/// A quiz of `questions` views behind a Next button; with `stuck`, Next does nothing.
struct Quiz {
    question: Mutex<usize>,
    stuck: bool,
}

impl Tool for Quiz {
    fn name(&self) -> &str {
        "quiz"
    }
    fn description(&self) -> &str {
        "a quiz"
    }
    fn start(&self, _: &str) -> ToolResult<()> {
        Ok(())
    }
    fn observe(&self) -> ToolResult<Observation> {
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
            fields: vec![],
        })
    }
    fn act(&self, _: &str, _: &Action) -> ToolResult<()> {
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
    let toolbox = Toolbox::default();
    toolbox.add(std::sync::Arc::new(quiz));
    DecisionLoop {
        toolbox: &toolbox,
        decider: &Taker,
        on_step: &|_| {},
        max_actions: 100,
    }
    .run("Answer all 25 questions", &[])
    .run
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

/// A tool that shows one still view at `location`.
struct Still(&'static str);

impl Tool for Still {
    fn name(&self) -> &str {
        self.0
    }
    fn description(&self) -> &str {
        "a still view"
    }
    fn start(&self, _: &str) -> ToolResult<()> {
        Ok(())
    }
    fn observe(&self) -> ToolResult<Observation> {
        Ok(Observation {
            outline: String::new(),
            epoch: "1".into(),
            location: self.0.into(),
            ..Observation::default()
        })
    }
    fn act(&self, _: &str, _: &Action) -> ToolResult<()> {
        Ok(())
    }
}

/// Picks `maps` for the goal; is BLOCKED in `maps` and DONE anywhere else.
struct Router;

impl Decider for Router {
    fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision, DecideError> {
        let answers = if questions.contains_key("tool") {
            [("tool".to_string(), sure("maps"))].into()
        } else if state["view"]["location"] == "maps" {
            [("operation".to_string(), sure("BLOCKED"))].into()
        } else {
            [("operation".to_string(), sure("DONE"))].into()
        };
        Ok(Decision {
            answers,
            ..Decision::default()
        })
    }
}

#[test]
fn the_model_picks_a_tool_and_a_blocked_one_hands_the_goal_on() {
    let toolbox = Toolbox::default();
    toolbox.add(std::sync::Arc::new(Still("web")));
    toolbox.add(std::sync::Arc::new(Still("maps")));
    let outcome = DecisionLoop {
        toolbox: &toolbox,
        decider: &Router,
        on_step: &|_| {},
        max_actions: 10,
    }
    .run("Find the route", &[]);
    assert_eq!(outcome.tool.as_deref(), Some("web"));
    assert_eq!(outcome.run.stop, Stop::Done { probability: 1.0 });
}

#[test]
fn an_empty_toolbox_blocks() {
    let outcome = DecisionLoop {
        toolbox: &Toolbox::default(),
        decider: &Router,
        on_step: &|_| {},
        max_actions: 10,
    }
    .run("Anything", &[]);
    assert_eq!((outcome.tool, outcome.run.stop), (None, Stop::Blocked));
}
