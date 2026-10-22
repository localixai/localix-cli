//! What a run spent: every call to either model goes through a meter, so the report counts all of
//! them (sub-goals, checks, the closing judgement), not only the ones the planner loop sees.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde_json::{Value, json};

use crate::decision::{Decider, Decision, Question};
use crate::openrouter::ApiError;
use crate::planner::{Planner, Reply};

/// One model's calls, tokens, time and money.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spend {
    /// The model that answered, as OpenRouter names it (`typesafe/jev-1.14-…` for an alias).
    pub model: String,
    pub calls: usize,
    pub input_tokens: u64,
    /// Of the input, how much came from the provider's prompt cache.
    pub cached_tokens: u64,
    pub output_tokens: u64,
    /// US dollars, as OpenRouter bills them.
    pub cost: f64,
    pub millis: u64,
}

impl Spend {
    fn add(&mut self, model: &str, input: u64, cached: u64, output: u64, cost: f64, millis: u64) {
        if !model.is_empty() {
            self.model = model.to_string();
        }
        self.calls += 1;
        self.input_tokens += input;
        self.cached_tokens += cached;
        self.output_tokens += output;
        self.cost += cost;
        self.millis += millis;
    }

    pub fn json(&self) -> Value {
        json!({
            "model": self.model,
            "calls": self.calls,
            "input_tokens": self.input_tokens,
            "cached_tokens": self.cached_tokens,
            "output_tokens": self.output_tokens,
            "cost_usd": self.cost,
            "ms": self.millis,
        })
    }
}

#[derive(Default)]
pub struct Meter(Mutex<Spend>);

impl Meter {
    pub fn spend(&self) -> Spend {
        self.0.lock().unwrap().clone()
    }
}

pub struct MeteredPlanner<'a> {
    pub inner: &'a dyn Planner,
    pub meter: &'a Meter,
}

impl MeteredPlanner<'_> {
    fn count(&self, reply: Result<Reply, ApiError>) -> Result<Reply, ApiError> {
        let reply = reply?;
        if std::env::var_os("LXSURF_DEBUG").is_some() {
            eprintln!(
                "planner call: {} ms · {} in ({} cached) · {} out",
                reply.millis, reply.input_tokens, reply.cached_tokens, reply.output_tokens
            );
        }
        self.meter.0.lock().unwrap().add(
            &reply.model,
            reply.input_tokens,
            reply.cached_tokens,
            reply.output_tokens,
            reply.cost,
            reply.millis,
        );
        Ok(reply)
    }
}

impl Planner for MeteredPlanner<'_> {
    fn turn(&self, messages: &[Value], tools: &Value) -> Result<Reply, ApiError> {
        self.count(self.inner.turn(messages, tools))
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
    ) -> Result<Decision, ApiError> {
        let decision = self.inner.decide(state, questions)?;
        self.meter.0.lock().unwrap().add(
            &decision.model,
            decision.input_tokens,
            0,
            decision.output_tokens,
            decision.cost,
            decision.millis,
        );
        Ok(decision)
    }
}

/// `12,345`
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spend_adds_up_and_reads_well() {
        let mut spend = Spend::default();
        spend.add("m", 1000, 800, 50, 0.001, 900);
        spend.add("", 2000, 0, 70, 0.002, 1100);
        assert_eq!(
            (
                spend.calls,
                spend.input_tokens,
                spend.cached_tokens,
                spend.output_tokens
            ),
            (2, 3000, 800, 120)
        );
        assert!((spend.cost - 0.003).abs() < 1e-12);
        assert_eq!(
            spend.model, "m",
            "an answer without a model keeps the last one"
        );
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(thousands(12), "12");
    }
}
