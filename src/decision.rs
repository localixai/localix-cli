//! The decision model: typed questions about a state, answered with calibrated probabilities
//! (OpenRouter's `/systemone`, Jev by default). It never writes text, so its output can only ever be
//! one of the options it was offered.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::openrouter::{ApiError, OpenRouter};

pub const DEFAULT_MODEL: &str = "~typesafe/jev-latest";

/// One question: pick one key of `criteria`, or say how likely a statement is.
#[derive(Debug, Clone)]
pub enum Question {
    Choice {
        instructions: Value,
        criteria: Map<String, Value>,
    },
    Noul {
        instructions: Value,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Choice {
        choice: String,
        probabilities: BTreeMap<String, f64>,
        confidence: f64,
    },
    Noul(f64),
}

#[derive(Debug, Clone, Default)]
pub struct Decision {
    pub answers: BTreeMap<String, Answer>,
    pub model: String,
    pub cost: f64,
    pub millis: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

pub trait Decider: Send + Sync {
    fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision, ApiError>;
}

pub struct SystemOne<'a> {
    pub client: &'a OpenRouter,
    pub model: String,
}

impl Decider for SystemOne<'_> {
    fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision, ApiError> {
        let questions: Map<String, Value> = questions
            .iter()
            .map(|(key, question)| {
                let body = match question {
                    Question::Choice { instructions, criteria } => {
                        json!({"type": "choice", "instructions": instructions, "criteria": criteria})
                    }
                    Question::Noul { instructions } => json!({"type": "noul", "instructions": instructions}),
                };
                (key.clone(), body)
            })
            .collect();
        let body = json!({"model": self.model, "state": state, "questions": questions});
        let (value, timing) = self.client.post("/systemone", &body)?;
        let mut decision = Decision {
            model: value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(&self.model)
                .to_string(),
            cost: value
                .pointer("/usage/cost")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            millis: timing.millis,
            input_tokens: value
                .pointer("/usage/input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            output_tokens: value
                .pointer("/usage/output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            ..Decision::default()
        };
        let answers = value
            .get("answers")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        for (key, answer) in answers {
            if let Some(answer) = read_answer(&answer) {
                decision.answers.insert(key, answer);
            }
        }
        Ok(decision)
    }
}

fn read_answer(answer: &Value) -> Option<Answer> {
    match answer.get("type")?.as_str()? {
        "choice" => Some(Answer::Choice {
            choice: answer.get("choice")?.as_str()?.to_string(),
            probabilities: answer
                .get("probabilities")?
                .as_object()?
                .iter()
                .filter_map(|(key, value)| Some((key.clone(), value.as_f64()?)))
                .collect(),
            confidence: answer
                .get("confidence")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        }),
        "noul" => Some(Answer::Noul(answer.get("noul")?.as_f64()?)),
        _ => None,
    }
}

/// The choice and its probability when the answer is one of `offered` and its distribution is
/// sound: finite, summing to one, with the choice on top. Anything else executes nothing
/// (jev-ultrafast's `validate_choice`).
pub fn valid_choice<'a>(
    answer: Option<&'a Answer>,
    offered: &Map<String, Value>,
) -> Option<(&'a str, f64)> {
    let Answer::Choice {
        choice,
        probabilities,
        ..
    } = answer?
    else {
        return None;
    };
    if !offered.contains_key(choice) {
        return None;
    }
    let sound = probabilities
        .values()
        .all(|p| p.is_finite() && (0.0..=1.0).contains(p))
        && (probabilities.values().sum::<f64>() - 1.0).abs() < 0.02
        && probabilities.keys().all(|key| offered.contains_key(key));
    let top = probabilities.values().cloned().fold(0.0, f64::max);
    let mine = *probabilities.get(choice)?;
    (sound && mine >= top - 1e-6).then_some((choice.as_str(), mine))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(keys: &[&str]) -> Map<String, Value> {
        keys.iter()
            .map(|key| (key.to_string(), json!(key)))
            .collect()
    }

    fn choice(choice: &str, probabilities: &[(&str, f64)]) -> Answer {
        Answer::Choice {
            choice: choice.into(),
            probabilities: probabilities
                .iter()
                .map(|(k, p)| (k.to_string(), *p))
                .collect(),
            confidence: 1.0,
        }
    }

    #[test]
    fn accepts_only_a_sound_choice_among_the_offered_keys() {
        let keys = offered(&["CLICK", "DONE"]);
        let good = choice("CLICK", &[("CLICK", 0.9), ("DONE", 0.1)]);
        assert_eq!(valid_choice(Some(&good), &keys), Some(("CLICK", 0.9)));
        let unknown = choice("TYPE", &[("TYPE", 1.0)]);
        assert_eq!(valid_choice(Some(&unknown), &keys), None);
        let not_top = choice("DONE", &[("CLICK", 0.9), ("DONE", 0.1)]);
        assert_eq!(valid_choice(Some(&not_top), &keys), None);
        let unsound = choice("CLICK", &[("CLICK", 0.9), ("DONE", 0.5)]);
        assert_eq!(valid_choice(Some(&unsound), &keys), None);
        assert_eq!(valid_choice(Some(&Answer::Noul(0.9)), &keys), None);
    }

    #[test]
    fn reads_the_systemone_answer_shapes() {
        let answer = json!({"type": "choice", "choice": "A", "probabilities": {"A": 1, "B": 0}, "confidence": 1});
        assert_eq!(
            read_answer(&answer),
            Some(choice("A", &[("A", 1.0), ("B", 0.0)]))
        );
        assert_eq!(
            read_answer(&json!({"type": "noul", "noul": 0.99})),
            Some(Answer::Noul(0.99))
        );
        assert_eq!(read_answer(&json!({"type": "score"})), None);
    }
}
