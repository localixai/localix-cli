//! What a run spent: every call to the decision model goes through a meter, so the report counts
//! all of them.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use serde_json::Value;

use contracts::chat::{Spend, Usage};
use contracts::model::DecideError;
use contracts::model::{Decider, Decision, Question};

#[derive(Default)]
pub struct Meter(Mutex<Spend>);

impl Meter {
    pub fn spend(&self) -> Spend {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
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
        self.meter
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .add(&Usage {
                model: decision.model.clone(),
                cost: decision.cost,
                millis: decision.millis,
                input_tokens: decision.input_tokens,
                output_tokens: decision.output_tokens,
                ..Usage::default()
            });
        Ok(decision)
    }
}
