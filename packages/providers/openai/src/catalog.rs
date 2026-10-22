//! The models a ChatGPT plan reaches through the ChatGPT backend, with the reasoning levels each
//! takes and the one it starts at, as the backend publishes them.

use contracts::catalog::{Effort, ModelEntry};

use Effort::{High, Low, Max, Medium, Ultra, XHigh};

/// The model a first sign-in starts with: quick and light on the plan's limits.
pub const DEFAULT_MODEL: &str = "gpt-6-luna";

const TO_ULTRA: &[Effort] = &[Low, Medium, High, XHigh, Max, Ultra];
const TO_MAX: &[Effort] = &[Low, Medium, High, XHigh, Max];
const TO_XHIGH: &[Effort] = &[Low, Medium, High, XHigh];

/// (id, name, levels, default level).
const MODELS: &[(&str, &str, &[Effort], Effort)] = &[
    ("gpt-6-luna", "GPT-6 Luna", TO_MAX, Medium),
    ("gpt-6-sol", "GPT-6 Sol", TO_ULTRA, Medium),
    ("gpt-6.1-sol", "GPT-6.1 Sol", TO_ULTRA, Low),
    ("gpt-6-astra", "GPT-6 Astra", TO_ULTRA, Low),
    ("gpt-5.6-luna", "GPT-5.6 Luna", TO_MAX, Medium),
    ("gpt-5.6-sol", "GPT-5.6 Sol", TO_ULTRA, Low),
    ("gpt-5.6-terra", "GPT-5.6 Terra", TO_ULTRA, Medium),
    ("gpt-5.5", "GPT-5.5", TO_XHIGH, Medium),
];

pub fn models() -> Vec<ModelEntry> {
    MODELS
        .iter()
        .map(|(id, name, efforts, default)| ModelEntry {
            id: id.to_string(),
            name: name.to_string(),
            efforts: efforts.to_vec(),
            default: *default,
        })
        .collect()
}

/// The entry for `id`; one this list does not know runs at the backend's own level, so it is
/// offered none.
pub fn entry(id: &str) -> ModelEntry {
    models()
        .into_iter()
        .find(|model| model.id == id)
        .unwrap_or_else(|| ModelEntry {
            id: id.to_string(),
            name: id.to_string(),
            efforts: vec![],
            default: Effort::Off,
        })
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
