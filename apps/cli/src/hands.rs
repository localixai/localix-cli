//! What the app gives the hands that the core may not do itself (ARCHITECTURE.md C1, C2): the
//! pause between two looks at a view, and, when a developer asks with `DECISION_LOOP_DEBUG=1`
//! (AGENTS.md), each decision's state and answers on stderr, read where the decision model is.

use std::collections::BTreeMap;
use std::time::Duration;

use contracts::model::{DecideError, Decider, Decision, Question};
use serde_json::Value;

/// The variable that asks for the decisions' trace.
const DEBUG: &str = "DECISION_LOOP_DEBUG";

/// Wait `time` before a view is looked at again.
#[allow(
    clippy::disallowed_methods,
    reason = "a poll: a view has no change event"
)]
pub(crate) fn pause(time: Duration) {
    std::thread::sleep(time);
}

/// The decision model, each of its decisions traced when the environment asks for it.
pub(crate) struct Traced<'a> {
    inner: &'a dyn Decider,
    asked: bool,
}

impl<'a> Traced<'a> {
    pub(crate) fn new(inner: &'a dyn Decider) -> Self {
        let asked = std::env::var_os(DEBUG).is_some();
        Self { inner, asked }
    }
}

impl Decider for Traced<'_> {
    fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision, DecideError> {
        if !self.asked {
            return self.inner.decide(state, questions);
        }
        trace(&format!("decision state: {state}"));
        let decision = self.inner.decide(state, questions)?;
        trace(&format!("decision answers: {:?}", decision.answers));
        Ok(decision)
    }
}

#[allow(
    clippy::print_stderr,
    reason = "a developer's trace, asked for by the environment, beside the interface"
)]
fn trace(line: &str) {
    eprintln!("{line}");
}
