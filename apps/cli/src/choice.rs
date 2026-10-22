//! What the agent thinks with: a provider, one of its models, and a reasoning level that model
//! takes. `/login` saves it as the default for later sessions (`<config>/llx/settings.json`);
//! `--provider`, `--model` and `--reasoning` override it for one run.

use std::path::PathBuf;

use contracts::catalog::{Effort, Selection};
use serde_json::{Value, json};

use crate::cli::ModelOptions;
use crate::compose::APP;
use crate::error::Failure;
use crate::providers::Provider;

const FILE: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub provider: Provider,
    pub selection: Selection,
}

impl Choice {
    /// `provider`'s model `id` at its default level.
    pub fn new(provider: Provider, id: &str) -> Self {
        Self {
            provider,
            selection: Selection::new(provider.entry(id)),
        }
    }

    pub fn model(&self) -> &str {
        &self.selection.model().id
    }

    /// `openai · gpt-6-sol · medium`.
    pub fn label(&self) -> String {
        format!(
            "{} · {} · {}",
            self.provider.id(),
            self.model(),
            self.selection.effort().name()
        )
    }

    fn to_json(&self) -> Value {
        json!({"provider": self.provider.id(), "model": self.model(),
            "reasoning": self.selection.effort().name()})
    }

    /// A saved choice; a level its model no longer takes falls back to the model's default.
    fn from_json(value: &Value) -> Option<Self> {
        let provider = Provider::named(value.get("provider")?.as_str()?)?;
        let choice = Self::new(provider, value.get("model")?.as_str()?);
        let effort = value
            .get("reasoning")
            .and_then(Value::as_str)
            .and_then(Effort::named);
        let selection = effort.and_then(|effort| choice.selection.with_effort(effort));
        Some(Self {
            selection: selection.unwrap_or(choice.selection),
            ..choice
        })
    }
}

fn path() -> Option<PathBuf> {
    provider_kit::store::path(APP, FILE)
}

/// The choice `/login` saved.
pub fn saved() -> Option<Choice> {
    let text = std::fs::read_to_string(path()?).ok()?;
    Choice::from_json(&serde_json::from_str(&text).ok()?)
}

pub fn save(choice: &Choice) -> std::io::Result<PathBuf> {
    let path = path().ok_or_else(|| std::io::Error::other("no home directory"))?;
    provider_kit::store::save(&path, &format!("{:#}\n", choice.to_json()))?;
    Ok(path)
}

/// The choice a run starts with: the options, else what was saved, else the first provider
/// signed in to (OpenRouter when none is) with its default model.
pub fn resolve(options: &ModelOptions, saved: Option<Choice>) -> Result<Choice, Failure> {
    let provider = options
        .provider
        .or(saved.as_ref().map(|choice| choice.provider))
        .unwrap_or_else(|| {
            let signed = [Provider::OpenRouter, Provider::OpenAi];
            let first = signed.into_iter().find(|p| p.signed_in().is_some());
            first.unwrap_or(Provider::OpenRouter)
        });
    let saved = saved.filter(|choice| choice.provider == provider);
    let choice = match (&options.model, saved) {
        (Some(model), Some(saved)) if saved.model() == model => saved,
        (Some(model), _) => Choice::new(provider, model),
        (None, Some(saved)) => saved,
        (None, None) => Choice::new(provider, provider.default_model()),
    };
    let Some(name) = &options.reasoning else {
        return Ok(choice);
    };
    let effort = Effort::named(name);
    match effort.and_then(|effort| choice.selection.with_effort(effort)) {
        Some(selection) => Ok(Choice {
            selection,
            ..choice
        }),
        None => Err(Failure::Usage(format!(
            "--reasoning {name}: {} takes {}",
            choice.model(),
            levels(&choice.selection)
        ))),
    }
}

/// The levels a selection's model takes, for a message: `low, medium, high`.
pub fn levels(selection: &Selection) -> String {
    let offered = selection.model().offered().iter().map(|e| e.name());
    offered.collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
#[path = "choice_tests.rs"]
mod tests;
