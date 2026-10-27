//! The decision port over OpenRouter's `/systemone`: any of its decision models (the app names
//! which).

use std::collections::BTreeMap;

use contracts::model::{Answer, DecideError, Decider, Decision, Question};
use serde_json::{Map, Value, json};

use crate::client::{ApiError, OpenRouter};

pub struct SystemOne {
    pub client: OpenRouter,
    pub model: String,
}

impl Decider for SystemOne {
    fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision, DecideError> {
        let questions: Map<String, Value> = questions
            .iter()
            .map(|(key, question)| {
                let body = match question {
                    Question::Choice {
                        instructions,
                        criteria,
                    } => {
                        json!({"type": "choice", "instructions": instructions, "criteria": criteria})
                    }
                    Question::Noul { instructions } => {
                        json!({"type": "noul", "instructions": instructions})
                    }
                };
                (key.clone(), body)
            })
            .collect();
        let body = json!({"model": self.model, "state": state, "questions": questions});
        let (value, timing) =
            self.client
                .post("/systemone", &body)
                .map_err(|error| match error {
                    ApiError::Invalid(message) => DecideError::Invalid(message),
                    other @ (ApiError::Auth(_) | ApiError::Http(..) | ApiError::Network(_)) => {
                        DecideError::Unavailable(format!("{} {other}", crate::NAME))
                    }
                })?;
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

/// An answer as the `/systemone` wire carries it.
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

#[cfg(test)]
#[path = "system_one_tests.rs"]
mod tests;
