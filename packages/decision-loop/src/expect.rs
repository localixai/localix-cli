//! Expectations: statements checked against the final view. The decision model answers each as a
//! probability (a `noul`); a statement holds only when it is clearly true.

use std::collections::BTreeMap;

use serde_json::json;

use agent::model::{Answer, Decider, Question};

/// At or above: true. Below, a statement the view does not clearly show is false.
pub const PASS: f64 = 0.8;

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub text: String,
    pub passed: bool,
    pub probability: Option<f64>,
}

pub fn check(expects: &[String], view: &str, decider: &dyn Decider) -> Vec<Verdict> {
    if expects.is_empty() {
        return vec![];
    }
    let questions: BTreeMap<String, Question> = expects
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let instructions = json!(format!("Does the view show that this is true? {text}"));
            (format!("x{}", i + 1), Question::Noul { instructions })
        })
        .collect();
    let state = json!({"view": view});
    let answers = decider
        .decide(&state, &questions)
        .map(|d| d.answers)
        .unwrap_or_default();
    expects
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let probability = match answers.get(&format!("x{}", i + 1)) {
                Some(Answer::Noul(p)) => Some(*p),
                _ => None,
            };
            Verdict {
                text: text.clone(),
                passed: probability.is_some_and(|p| p >= PASS),
                probability,
            }
        })
        .collect()
}
