//! A provider's sign-in in the browser, for `/login`: with `providers.rs`, the one place that
//! knows each provider package's own (K1, A5). Where the person is sent, how the code comes
//! back, and what keeping it says, with no secret in the words.

use std::time::Duration;

use provider_kit::http::Web;
use provider_kit::store::masked;

use super::{Provider, Vars};
use crate::compose::APP;

/// How long the token endpoint may take.
const TOKEN_TIMEOUT: Duration = Duration::from_secs(60);

/// A browser sign-in begun.
pub(crate) trait Login {
    /// Where the person is sent.
    fn address(&self) -> &str;
    /// The code, once the browser came back within `timeout`.
    fn wait(&self, timeout: Duration) -> Option<Result<String, String>>;
    /// The code in the address the browser ended on, pasted.
    fn pasted(&self, input: &str) -> Result<String, String>;
    /// The code exchanged and kept: what was saved, in words with no secret in them.
    fn finish(&self, code: &str) -> Result<String, String>;
}

impl Provider {
    /// A sign-in in the browser begun, where the provider has one; `vars` is the session's
    /// environment, where a server standing in for OpenRouter is named.
    pub(crate) fn browser_login(&self, vars: Vars) -> Option<Result<Box<dyn Login>, String>> {
        let begun: Result<Box<dyn Login>, String> = match self.id() {
            "openai" => openai::auth::Login::start(APP).map(|login| Box::new(ChatGpt(login)) as _),
            "openrouter" => openrouter::auth::Login::start(APP).map(|login| {
                let client = super::openrouter_client(String::new(), vars);
                Box::new(OpenRouter(login, client)) as _
            }),
            _ => return None,
        };
        Some(begun)
    }
}

/// ChatGPT's sign-in: its tokens, kept.
struct ChatGpt(openai::auth::Login);

impl Login for ChatGpt {
    fn address(&self) -> &str {
        self.0.address()
    }

    fn wait(&self, timeout: Duration) -> Option<Result<String, String>> {
        self.0.wait(timeout)
    }

    fn pasted(&self, input: &str) -> Result<String, String> {
        self.0.pasted(input)
    }

    fn finish(&self, code: &str) -> Result<String, String> {
        let tokens = self.0.finish(&Web::new(APP, TOKEN_TIMEOUT), code)?;
        let file = openai::tokens::save(APP, &tokens).map_err(|e| e.to_string())?;
        Ok(format!(
            "Signed in to ChatGPT account {}; saved in {}.",
            tokens.account,
            file.display()
        ))
    }
}

/// OpenRouter's sign-in: a key made for llx, kept.
struct OpenRouter(openrouter::auth::Login, openrouter::client::OpenRouter);

impl Login for OpenRouter {
    fn address(&self) -> &str {
        self.0.address()
    }

    fn wait(&self, timeout: Duration) -> Option<Result<String, String>> {
        self.0.wait(timeout)
    }

    fn pasted(&self, input: &str) -> Result<String, String> {
        self.0.pasted(input)
    }

    fn finish(&self, code: &str) -> Result<String, String> {
        let key = self.0.finish(&self.1, code)?;
        let file = Provider::openrouter()
            .save_key(&key)
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "Signed in: the key {} is saved in {}.",
            masked(&key),
            file.display()
        ))
    }
}
