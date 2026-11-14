//! `--schema` as the agent checks it: flowschema behind the `Shape` port (ARCHITECTURE.md P8).

use contracts::shape::Shape;
use flowschema::Schema;
use serde_json::Value;

/// The schema `--schema` gives, as it was written and as it is checked.
pub(crate) struct Checked {
    json: Value,
    compiled: Schema,
}

impl Checked {
    pub(crate) fn new(json: Value) -> Self {
        let compiled = Schema::new(&json);
        Self { json, compiled }
    }

    /// What `--schema` says data is not held to here, said as the run starts: the model is told
    /// those keywords, and a reader of the data should not count on them.
    pub(crate) fn unchecked_note(&self) -> Option<String> {
        let unchecked = self.compiled.unchecked();
        let (are, them) = match unchecked.len() {
            1 => ("is", "it"),
            _ => ("are", "them"),
        };
        (!unchecked.is_empty()).then(|| {
            let names = unchecked.join(", ");
            format!(
                "llx: --schema: {names} {are} not checked (the model is told {them}; the data is not held to {them})\n"
            )
        })
    }
}

impl Shape for Checked {
    fn schema(&self) -> &Value {
        &self.json
    }

    fn mismatches(&self, data: &Value) -> Vec<String> {
        self.compiled.mismatches(data)
    }
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
