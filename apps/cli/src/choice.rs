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
pub(crate) struct Choice {
    pub provider: Provider,
    pub selection: Selection,
}

impl Choice {
    /// `provider`'s model `id` at its default level.
    pub(crate) fn new(provider: Provider, id: &str) -> Self {
        Self {
            selection: Selection::new(provider.entry(id)),
            provider,
        }
    }

    pub(crate) fn model(&self) -> &str {
        &self.selection.model().id
    }

    /// `openai · gpt-6-sol · medium`.
    pub(crate) fn label(&self) -> String {
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

/// `<config>/llx/settings.json`: the saved choice and the person's modes.
pub(crate) fn settings_path() -> Option<PathBuf> {
    provider_kit::store::path(APP, FILE)
}

fn path() -> Option<PathBuf> {
    settings_path()
}

/// The settings file's text with `choice` put in, the rest (the person's modes) kept; text that
/// is not a JSON object is replaced.
fn merged(existing: Option<&str>, choice: &Choice) -> String {
    let mut settings = existing
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    if let (Some(settings), Value::Object(chosen)) = (settings.as_object_mut(), choice.to_json()) {
        settings.extend(chosen);
    }
    format!("{settings:#}\n")
}

/// The choice `/login` saved.
pub(crate) fn saved() -> Option<Choice> {
    let text = std::fs::read_to_string(path()?).ok()?;
    Choice::from_json(&serde_json::from_str(&text).ok()?)
}

pub(crate) fn save(choice: &Choice) -> std::io::Result<PathBuf> {
    let path = path().ok_or_else(|| std::io::Error::other("no home directory"))?;
    let existing = std::fs::read_to_string(&path).ok();
    provider_kit::store::save(&path, &merged(existing.as_deref(), choice))?;
    Ok(path)
}

/// The choice a run starts with: the options, else what was saved, else the first provider
/// signed in to (OpenRouter when none is) with its default model.
pub(crate) fn resolve(
    options: &ModelOptions,
    saved: Option<Choice>,
    vars: crate::providers::Vars,
) -> Result<Choice, Failure> {
    let provider = match &options.provider {
        Some(id) => Provider::named(id).ok_or_else(|| {
            Failure::Usage(format!(
                "--provider {id}: not a provider llx knows (/login lists them)"
            ))
        })?,
        None => match &saved {
            Some(saved) => saved.provider.clone(),
            None => {
                let first = [Provider::openrouter(), Provider::openai()];
                let first = first.into_iter().chain(Provider::all());
                let signed = first.into_iter().find(|p| p.signed_in_by(vars).is_some());
                signed.unwrap_or_else(Provider::openrouter)
            }
        },
    };
    let saved = saved.filter(|choice| choice.provider == provider);
    let choice = match (&options.model, saved) {
        (Some(model), Some(saved)) if saved.model() == model => saved,
        (Some(model), _) => Choice::new(provider, model),
        (None, Some(saved)) => saved,
        (None, None) => {
            let model = provider.default_model();
            Choice::new(provider, &model)
        }
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
pub(crate) fn levels(selection: &Selection) -> String {
    let offered = selection.model().offered().iter().map(|e| e.name());
    offered.collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
#[path = "choice_tests.rs"]
mod tests;
