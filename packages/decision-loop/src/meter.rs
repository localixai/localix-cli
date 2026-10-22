//! What a run spent: every call to the decision model goes through a meter, so the report counts
//! all of them.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::Value;

use contracts::model::DecideError;
use contracts::model::{Decider, Decision, Question};

/// One model's calls, tokens, time and money.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spend {
    /// The model that answered, as its provider names it (the version behind an alias).
    pub model: String,
    pub calls: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// US dollars, as the provider bills them.
    pub cost: f64,
    pub millis: u64,
}

impl Spend {
    fn add(&mut self, model: &str, input: u64, output: u64, cost: f64, millis: u64) {
        if !model.is_empty() {
            self.model = model.to_string();
        }
        self.calls += 1;
        self.input_tokens += input;
        self.output_tokens += output;
        self.cost += cost;
        self.millis += millis;
    }
}

#[derive(Default)]
pub struct Meter(Mutex<Spend>);

impl Meter {
    pub fn spend(&self) -> Spend {
        self.0.lock().unwrap().clone()
    }
}

pub struct MeteredDecider<'a> {
    pub inner: &'a dyn Decider,
    pub meter: &'a Meter,
}

impl Decider for MeteredDecider<'_> {
    fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision, DecideError> {
        let decision = self.inner.decide(state, questions)?;
        self.meter.0.lock().unwrap().add(
            &decision.model,
            decision.input_tokens,
            decision.output_tokens,
            decision.cost,
            decision.millis,
        );
        Ok(decision)
    }
}

#[cfg(test)]
#[path = "meter_tests.rs"]
mod tests;
