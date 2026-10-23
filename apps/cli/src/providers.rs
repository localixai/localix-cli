//! The providers llx can think with: every one the model catalog lists (`models`, from
//! models.dev), and the one place that knows the provider packages: how each is signed in to,
//! where its key is kept, and how its chat model is built. OpenAI is the ChatGPT backend with a
//! ChatGPT sign-in, else its API with a key; every other provider is called over the OpenAI
//! chat completions at the address the catalog gives (OpenRouter with its own extras).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use contracts::catalog::{Effort, ModelEntry, Selection};
use contracts::chat::{Chat, ChatError, Completion, Request};
use contracts::model::Decider;
use models::{CHAT_COMPLETIONS, Catalog, RESPONSES};

use crate::compose::APP;

/// A provider, by its catalog id (`openai`, `openrouter`, `deepseek`…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provider(pub String);

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
/// The model a provider starts with, where llx has a view: a policy, by catalog id.
const DEFAULTS: [(&str, &str); 2] = [
    ("openai", "gpt-6-luna"),
    ("openrouter", "anthropic/claude-haiku-5.5"),
];

/// The browser's decision model llx starts with: a policy, by catalog id (OpenRouter's).
pub const DECISION_DEFAULT: &str = "~typesafe/jev-latest";

/// The catalog: the built-in files under the last fetched one, read once (unit tests see the
/// built-in files only, the same on every machine).
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| match cfg!(test) {
        true => Catalog::built_in(),
        false => models::source::load(APP),
    })
}

impl Provider {
    pub fn openai() -> Self {
        Self("openai".into())
    }

    pub fn openrouter() -> Self {
        Self("openrouter".into())
    }

    /// Every provider llx can call, the popular first.
    pub fn all() -> Vec<Provider> {
        catalog()
            .ids()
            .into_iter()
            .map(|id| Self(id.into()))
            .collect()
    }

    /// The provider `id` names, when the catalog lists it.
    pub fn named(id: &str) -> Option<Self> {
        let id = id.trim().to_ascii_lowercase();
        catalog().ids().contains(&id.as_str()).then_some(Self(id))
    }

    pub fn id(&self) -> &str {
        &self.0
    }

    /// What the person reads.
    pub fn name(&self) -> String {
        catalog()
            .provider(&self.0)
            .map_or_else(|| self.0.clone(), |p| p.name.clone())
    }

    /// How it is signed in to, where that is not just a key: said where the provider is picked.
    pub fn hint(&self) -> Option<&'static str> {
        (self.0 == "openai").then_some("ChatGPT sign-in, or an API key")
    }

    pub fn methods(&self) -> &'static [Method] {
        match self.0.as_str() {
            "openai" => &[Method::Browser, Method::Key],
            "openrouter" => &[Method::Key, Method::Browser],
            _ => &[Method::Key],
        }
    }

    /// The models it is called with: over the ChatGPT backend with a ChatGPT sign-in, else over
    /// chat completions (unit tests read no sign-in: the same on every machine).
    pub fn models(&self) -> Vec<ModelEntry> {
        let chatgpt = self.0 == "openai" && !cfg!(test) && openai::tokens::load(APP).is_some();
        self.served(chatgpt)
    }

    /// Its models over the ChatGPT backend (`chatgpt`, the Responses API), else over chat
    /// completions.
    pub fn served(&self, chatgpt: bool) -> Vec<ModelEntry> {
        match chatgpt && self.0 == "openai" {
            true => catalog().served(&self.0, RESPONSES),
            false => catalog().models(&self.0),
        }
    }

    /// The entry for model `id`; one the catalog does not know is offered no level.
    pub fn entry(&self, id: &str) -> ModelEntry {
        catalog().entry(&self.0, id).unwrap_or_else(|| ModelEntry {
            id: id.to_string(),
            name: id.to_string(),
            efforts: vec![],
            default: Effort::Off,
            context: 0,
        })
    }

    /// llx's default where it has one and the catalog lists it, else the newest model.
    pub fn default_model(&self) -> String {
        let models = self.models();
        let stated = DEFAULTS.iter().find(|(p, _)| *p == self.0).map(|(_, m)| *m);
        match stated.filter(|id| models.iter().any(|m| m.id == *id)) {
            Some(id) => id.to_string(),
            None => models.first().map_or_else(String::new, |m| m.id.clone()),
        }
    }

    /// Where the sign-in comes from, in words with no secret in them; `None` when signed out.
    pub fn signed_in(&self) -> Option<String> {
        if self.0 == "openai"
            && let Some(tokens) = openai::tokens::load(APP)
        {
            return Some(format!("ChatGPT account {}", tokens.account));
        }
        self.key().map(|(_, from)| from)
    }

    /// The key and where it comes from: its variable (as the catalog names them), else the one
    /// saved for llx.
    pub fn key(&self) -> Option<(String, String)> {
        if let Some((name, key)) = self.variables().into_iter().find_map(|name| {
            let key = std::env::var(&name).ok().filter(|k| !k.trim().is_empty())?;
            Some((name, key.trim().to_string()))
        }) {
            return Some((key, name));
        }
        let path = self.key_path()?;
        let key = provider_kit::store::read(&path)?;
        Some((key, format!("the key in {}", path.display())))
    }

    /// What a run that is not signed in says: a session's /login, or the variable its key is
    /// read from.
    pub fn how_to_sign_in(&self) -> String {
        let login = format!(
            "not signed in to {}: open a session (`llx`) and type /login",
            self.name()
        );
        match self.variables().first() {
            Some(variable) => format!("{login}, or set {variable}"),
            None => login,
        }
    }

    /// Whether the key comes from the environment, which no saved one replaces.
    pub fn key_in_env(&self) -> bool {
        self.variables()
            .iter()
            .any(|name| std::env::var(name).is_ok_and(|k| !k.trim().is_empty()))
    }

    /// What commands never see: the variables its key is read from, and OpenRouter's, which
    /// the decision model is called with.
    pub fn hidden(&self) -> Vec<String> {
        let mut names = self.variables();
        names.retain(|name| name != openrouter::auth::KEY_VARIABLE);
        names.push(openrouter::auth::KEY_VARIABLE.to_string());
        names
    }

    fn variables(&self) -> Vec<String> {
        catalog()
            .provider(&self.0)
            .map_or_else(Vec::new, |p| p.env.clone())
    }

    /// Where its pasted key is kept (OpenRouter's where it always was).
    pub fn key_path(&self) -> Option<PathBuf> {
        match self.0.as_str() {
            "openrouter" => openrouter::auth::credentials_path(APP),
            id => provider_kit::store::path(APP, &format!("key-{id}")),
        }
    }

    /// Keep `key` for llx; where.
    pub fn save_key(&self, key: &str) -> std::io::Result<PathBuf> {
        let path = self
            .key_path()
            .ok_or_else(|| std::io::Error::other("no home directory"))?;
        provider_kit::store::save(&path, key)?;
        Ok(path)
    }

    /// The chat model for `selection`, each request priced; `None` when not signed in.
    pub fn chat(&self, selection: &Selection) -> Option<Box<dyn Chat>> {
        let inner = self.unpriced(selection)?;
        Some(Box::new(Priced {
            inner,
            provider: self.0.clone(),
            model: selection.model().id.clone(),
        }))
    }

    fn unpriced(&self, selection: &Selection) -> Option<Box<dyn Chat>> {
        let selection = selection.clone();
        if self.0 == "openai"
            && let Some(tokens) = openai::tokens::load(APP)
        {
            let http = provider_kit::http::Web::new(APP, OPENAI_TIMEOUT);
            let keep = openai::responses::Saved(APP);
            let model = openai::responses::ChatModel::new(
                APP,
                Box::new(http),
                Box::new(keep),
                tokens,
                selection,
            );
            return Some(Box::new(model));
        }
        let (key, _) = self.key()?;
        let client = match self.0.as_str() {
            "openrouter" => openrouter::client::OpenRouter::new(key, APP),
            id => {
                let model = catalog().model(id, &selection.model().id);
                let address = model
                    .and_then(|m| m.address(CHAT_COMPLETIONS))
                    .or_else(|| {
                        let first = catalog().provider(id)?.models.values();
                        first.filter_map(|m| m.address(CHAT_COMPLETIONS)).next()
                    })?;
                let name = catalog().provider(id).map_or(id, |p| p.name.as_str());
                openrouter::client::OpenRouter::at(address, key, APP, name)
            }
        };
        Some(Box::new(openrouter::chat::ChatModel { client, selection }))
    }

    /// Forget the saved sign-in; whether there was one.
    pub fn forget(&self) -> std::io::Result<bool> {
        let mut removed = false;
        if self.0 == "openai" {
            removed |= openai::tokens::remove(APP)?;
        }
        if let Some(path) = self.key_path() {
            removed |= provider_kit::store::remove(&path)?;
        }
        Ok(removed)
    }
}

/// A chat model whose requests are priced from the catalog when the provider reports no billed
/// amount: a ChatGPT plan's requests as their API-equivalent value.
struct Priced {
    inner: Box<dyn Chat>,
    provider: String,
    model: String,
}

impl Chat for Priced {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        let mut completion = self.inner.complete(request)?;
        let usage = &mut completion.usage;
        if usage.cost == 0.0
            && let Some(model) = catalog().model(&self.provider, &self.model)
        {
            let (input, cached, output) =
                (usage.input_tokens, usage.cached_tokens, usage.output_tokens);
            usage.cost = model.price(input, cached, output).unwrap_or(0.0);
        }
        Ok(completion)
    }
}

/// The browser's decision model: OpenRouter's, the one provider that serves decision models,
/// so it needs an OpenRouter key whichever provider the agent thinks with.
pub fn decider(model: &str) -> Option<Box<dyn Decider>> {
    let (key, _) = Provider::openrouter().key()?;
    let client = openrouter::client::OpenRouter::deciding(key, APP);
    Some(Box::new(openrouter::system_one::SystemOne {
        client,
        model: model.to_string(),
    }))
}

/// Ask models.dev for a newer catalog, at most once a day, on a thread of its own: the next
/// session reads what it keeps.
pub fn refresh_catalog() {
    // `LLX_MODELS_REFRESH=0`: no network for it (tests, offline machines).
    if std::env::var("LLX_MODELS_REFRESH").is_ok_and(|v| v == "0") {
        return;
    }
    std::thread::spawn(|| {
        let _ = models::source::refresh(APP);
    });
}

#[cfg(test)]
#[path = "providers_tests.rs"]
mod tests;
