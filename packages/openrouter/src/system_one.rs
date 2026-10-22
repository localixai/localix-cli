//! The decision port over OpenRouter's `/systemone`: Jev by default, or any of its decision models.

use std::collections::BTreeMap;

use agent::model::{Answer, DecideError, Decider, Decision, Question};
use serde_json::{Map, Value, json};

use crate::client::{ApiError, OpenRouter};

pub const DEFAULT_MODEL: &str = "~typesafe/jev-latest";

pub struct SystemOne<'a> {
    pub client: &'a OpenRouter,
    pub model: String,
}

impl Decider for SystemOne<'_> {
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
                    other => DecideError::Unavailable(format!("OpenRouter: {other}")),
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

/// OpenRouter's decision models, cheapest first: (id, price of a million input tokens in USD,
/// context in tokens). A decision costs mostly its input.
pub fn models(client: &OpenRouter) -> Result<Vec<(String, f64, u64)>, ApiError> {
    let listing = client.get("/models?output_modalities=decisions")?;
    let mut models: Vec<(String, f64, u64)> = listing
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|model| {
            let price = model
                .pointer("/pricing/prompt")
                .and_then(Value::as_str)
                .and_then(|p| p.parse::<f64>().ok())
                .map_or(f64::MAX, |p| p * 1e6);
            (
                model["id"].as_str().unwrap_or("").to_string(),
                price,
                model["context_length"].as_u64().unwrap_or(0),
            )
        })
        .collect();
    models.sort_by(|a, b| a.1.total_cmp(&b.1));
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_systemone_answer_shapes() {
        let answer = json!({"type": "choice", "choice": "A", "probabilities": {"A": 1, "B": 0}, "confidence": 1});
        assert_eq!(
            read_answer(&answer),
            Some(Answer::Choice {
                choice: "A".into(),
                probabilities: [("A".to_string(), 1.0), ("B".to_string(), 0.0)].into(),
                confidence: 1.0,
            })
        );
        assert_eq!(
            read_answer(&json!({"type": "noul", "noul": 0.99})),
            Some(Answer::Noul(0.99))
        );
        assert_eq!(read_answer(&json!({"type": "score"})), None);
    }
}
