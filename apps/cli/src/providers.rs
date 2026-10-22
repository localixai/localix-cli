//! The providers llx can think with, and the one place that knows each package: its name, its
//! models, where its sign-in is kept, and how its chat model is built. A new provider is a
//! package under `packages/providers/` and one arm in each `match` here.

use std::time::Duration;

use contracts::catalog::{ModelEntry, Selection};
use contracts::chat::Chat;
use contracts::model::Decider;
use openrouter::auth::KeySource;

use crate::compose::APP;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Provider {
    /// OpenAI's models with a ChatGPT sign-in (billed to the plan).
    #[value(name = "openai")]
    OpenAi,
    /// Any of OpenRouter's models, with an OpenRouter key.
    #[value(name = "openrouter")]
    OpenRouter,
}

/// How a provider is signed in to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// OAuth in the system browser, back to a local callback.
    Browser,
    /// A key the person pastes.
    Key,
}

/// How long the ChatGPT backend may take over one streamed reply (high reasoning is slow).
const OPENAI_TIMEOUT: Duration = Duration::from_secs(600);

impl Provider {
    pub const ALL: [Provider; 2] = [Provider::OpenAi, Provider::OpenRouter];

    pub fn id(self) -> &'static str {
        match self {
            Provider::OpenAi => "openai",
            Provider::OpenRouter => "openrouter",
        }
    }

    pub fn named(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|provider| provider.id() == id)
    }

    /// What the person reads.
    pub fn name(self) -> &'static str {
        match self {
            Provider::OpenAi => "OpenAI (sign in with ChatGPT)",
            Provider::OpenRouter => openrouter::NAME,
        }
    }

    pub fn methods(self) -> &'static [Method] {
        match self {
            Provider::OpenAi => &[Method::Browser],
            Provider::OpenRouter => &[Method::Key, Method::Browser],
        }
    }

    pub fn models(self) -> Vec<ModelEntry> {
        match self {
            Provider::OpenAi => openai::catalog::models(),
            Provider::OpenRouter => openrouter::catalog::models(),
        }
    }

    /// The entry for model `id`; one the catalog does not know is offered no reasoning level.
    pub fn entry(self, id: &str) -> ModelEntry {
        match self {
            Provider::OpenAi => openai::catalog::entry(id),
            Provider::OpenRouter => openrouter::catalog::entry(id),
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Provider::OpenAi => openai::catalog::DEFAULT_MODEL,
            Provider::OpenRouter => openrouter::catalog::DEFAULT_MODEL,
        }
    }

    /// Where the sign-in comes from, in words with no secret in them; `None` when signed out.
    pub fn signed_in(self) -> Option<String> {
        match self {
            Provider::OpenAi => openai::tokens::load(APP)
                .map(|tokens| format!("ChatGPT account {}", tokens.account)),
            Provider::OpenRouter => openrouter::auth::key(APP).map(|(_, source)| match source {
                KeySource::Env => openrouter::auth::KEY_VARIABLE.to_string(),
                KeySource::Saved(path) => format!("the key in {}", path.display()),
            }),
        }
    }

    /// Whether the sign-in comes from the environment, which no saved one replaces.
    pub fn key_in_env(self) -> bool {
        matches!(
            (self, openrouter::auth::key(APP)),
            (Provider::OpenRouter, Some((_, KeySource::Env)))
        )
    }

    /// The chat model for `selection`; `None` when not signed in.
    pub fn chat(self, selection: &Selection) -> Option<Box<dyn Chat>> {
        let selection = selection.clone();
        match self {
            Provider::OpenAi => {
                let tokens = openai::tokens::load(APP)?;
                let http = provider_kit::http::Web::new(APP, OPENAI_TIMEOUT);
                let keep = openai::responses::Saved(APP);
                let model = openai::responses::ChatModel::new(
                    APP,
                    Box::new(http),
                    Box::new(keep),
                    tokens,
                    selection,
                );
                Some(Box::new(model))
            }
            Provider::OpenRouter => {
                let (key, _) = openrouter::auth::key(APP)?;
                let client = openrouter::client::OpenRouter::new(key, APP);
                Some(Box::new(openrouter::chat::ChatModel { client, selection }))
            }
        }
    }

    /// Forget the saved sign-in; whether there was one.
    pub fn forget(self) -> std::io::Result<bool> {
        match self {
            Provider::OpenAi => openai::tokens::remove(APP),
            Provider::OpenRouter => openrouter::auth::remove(APP),
        }
    }
}

/// The browser's decision model: OpenRouter's, the one provider that serves decision models,
/// so it needs an OpenRouter key whichever provider the agent thinks with.
pub fn decider(model: &str) -> Option<Box<dyn Decider>> {
    let (key, _) = openrouter::auth::key(APP)?;
    let client = openrouter::client::OpenRouter::new(key, APP);
    Some(Box::new(openrouter::system_one::SystemOne {
        client,
        model: model.to_string(),
    }))
}

#[cfg(test)]
#[path = "providers_tests.rs"]
mod tests;
