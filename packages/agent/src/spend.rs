//! What the chat model spent in one run: its turns' usage, summed.

use contracts::chat::Usage;

/// What the chat model spent in one run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spend {
    pub model: String,
    pub calls: u64,
    pub cost: f64,
    pub millis: u64,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub output_tokens: u64,
}

impl Spend {
    pub(crate) fn add(&mut self, usage: &Usage) {
        self.model.clone_from(&usage.model);
        self.calls += 1;
        self.cost += usage.cost;
        self.millis += usage.millis;
        self.input_tokens += usage.input_tokens;
        self.cached_tokens += usage.cached_tokens;
        self.output_tokens += usage.output_tokens;
    }
}
