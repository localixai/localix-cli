//! A short list of OpenRouter's chat models that call tools well, with the reasoning levels each
//! takes as OpenRouter publishes them. Each starts with no reasoning where it can run without
//! (a turn costs seconds of thinking otherwise), else at its lowest level. Any other model id
//! still works (`--model`); it is offered no level, so it is asked for none.

use contracts::catalog::{Effort, ModelEntry};

use Effort::{High, Low, Max, Medium, Off, XHigh};

pub const DEFAULT_MODEL: &str = "anthropic/claude-haiku-5.5";

/// (id, name, levels).
const MODELS: &[(&str, &str, &[Effort])] = &[
    (
        "anthropic/claude-haiku-5.5",
        "Claude Haiku 5.5",
        &[Off, Low, Medium, High, XHigh, Max],
    ),
    (
        "anthropic/claude-sonnet-5.5",
        "Claude Sonnet 5.5",
        &[Low, Medium, High, XHigh, Max],
    ),
    (
        "anthropic/claude-opus-5.5",
        "Claude Opus 5.5",
        &[Low, Medium, High, XHigh, Max],
    ),
    (
        "openai/gpt-6-luna",
        "GPT-6 Luna",
        &[Off, Low, Medium, High, XHigh, Max],
    ),
    (
        "openai/gpt-6-sol",
        "GPT-6 Sol",
        &[Off, Low, Medium, High, XHigh, Max],
    ),
    (
        "google/gemini-3.8-flash",
        "Gemini 3.8 Flash",
        &[Low, Medium, High],
    ),
    (
        "google/gemini-4-argon",
        "Gemini 4 Argon",
        &[Low, Medium, High],
    ),
    ("x-ai/grok-4.7", "Grok 4.7", &[Low, Medium, High, XHigh]),
    (
        "deepseek/deepseek-v4.1-flash",
        "DeepSeek V4.1 Flash",
        &[Off, Low, High, Max],
    ),
    ("moonshotai/kimi-k3", "Kimi K3", &[Off, Low, High, Max]),
    ("z-ai/glm-5.3", "GLM 5.3", &[Low, High, Max]),
];

pub fn models() -> Vec<ModelEntry> {
    MODELS
        .iter()
        .map(|(id, name, efforts)| ModelEntry {
            id: id.to_string(),
            name: name.to_string(),
            efforts: efforts.to_vec(),
            default: efforts[0],
        })
        .collect()
}

/// The entry for `id`, or one that is offered no level for a model this list does not know.
pub fn entry(id: &str) -> ModelEntry {
    models()
        .into_iter()
        .find(|model| model.id == id)
        .unwrap_or_else(|| ModelEntry {
            id: id.to_string(),
            name: id.to_string(),
            efforts: vec![],
            default: Off,
        })
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
