//! Signing in to a provider, for `/login`, and `llx logout`, which forgets every saved sign-in.
//! Keys and tokens are never shown: a key is read with the echo off, and only its masked ends
//! are printed.

use std::path::PathBuf;
use std::time::Duration;

use provider_kit::http::Web;
use provider_kit::oauth;

use crate::compose::APP;
use crate::error::{self, Failure};
use crate::exit;
use crate::picker::Terminal;
use crate::providers::{Method, Provider};

/// How long the token endpoint may take.
const TOKEN_TIMEOUT: Duration = Duration::from_secs(60);

/// Sign in to `provider` with `method`; what was saved, in words with no secret in them.
pub fn sign_in(
    provider: Provider,
    method: Method,
    terminal: &mut dyn Terminal,
    say: &dyn Fn(&str),
) -> Result<String, String> {
    let saved = |path: PathBuf| format!("saved in {}", path.display());
    match (provider, method) {
        (Provider::OpenAi, _) => {
            let login = openai::auth::Login::start(APP)?;
            let code = browser(&login.address, terminal, say, &|t| login.wait(t), &|p| {
                login.pasted(p)
            })?;
            let tokens = login.finish(&Web::new(APP, TOKEN_TIMEOUT), &code)?;
            let path = openai::tokens::save(APP, &tokens).map_err(|e| e.to_string())?;
            let account = &tokens.account;
            Ok(format!(
                "Signed in to ChatGPT account {account}; {}.",
                saved(path)
            ))
        }
        (Provider::OpenRouter, Method::Browser) => {
            let login = openrouter::auth::Login::start(APP)?;
            let code = browser(&login.address, terminal, say, &|t| login.wait(t), &|p| {
                login.pasted(p)
            })?;
            let key = login.finish(APP, &code)?;
            let path = openrouter::auth::save(APP, &key).map_err(|e| e.to_string())?;
            Ok(format!(
                "Signed in: the key {} is {}.",
                mask(&key),
                saved(path)
            ))
        }
        (Provider::OpenRouter, Method::Key) => {
            let prompt = "Paste the OpenRouter key and press Enter (it is not shown): ";
            let line = terminal.line(prompt, true).ok_or("no key was pasted")?;
            let key = openrouter::auth::as_key(&line).ok_or("that is not a key")?;
            let path = openrouter::auth::save(APP, key).map_err(|e| e.to_string())?;
            let mut said = format!("Saved the key {}; {}.", mask(key), saved(path));
            if provider.key_in_env() {
                said.push_str(" OPENROUTER_API_KEY is set, and it wins over the saved key.");
            }
            Ok(said)
        }
    }
}

/// Open `address` in the system browser, and wait for it.
fn browser(
    address: &str,
    terminal: &mut dyn Terminal,
    say: &dyn Fn(&str),
    wait: &dyn Fn(Duration) -> Option<Result<String, String>>,
    pasted: &dyn Fn(&str) -> Result<String, String>,
) -> Result<String, String> {
    say(&format!(
        "Opening the sign-in in your browser:\n  {address}\n"
    ));
    oauth::open(address);
    come_back(terminal, say, wait, pasted)
}

/// Wait for the browser to come back, or for the address it ended on to be pasted (when it is
/// on another machine); the authorization code. The prompt line is read on this thread only, so
/// nothing is left reading stdin once the sign-in ends.
pub fn come_back(
    terminal: &mut dyn Terminal,
    say: &dyn Fn(&str),
    wait: &dyn Fn(Duration) -> Option<Result<String, String>>,
    pasted: &dyn Fn(&str) -> Result<String, String>,
) -> Result<String, String> {
    let prompt = "Press Enter once the browser says you are signed in, or paste the address \
                  it ended on (q to cancel) > ";
    loop {
        let line = terminal.line(prompt, false).ok_or("cancelled")?;
        match line.as_str() {
            "q" | "Q" => return Err("cancelled".into()),
            "" => match wait(Duration::from_secs(3)) {
                Some(outcome) => return outcome,
                None => say("The browser has not come back yet.\n"),
            },
            text => return pasted(text),
        }
    }
}

/// `llx logout`: forget every saved sign-in.
pub fn logout() -> Result<i32, Failure> {
    for provider in Provider::ALL {
        let removed = provider
            .forget()
            .map_err(|error| Failure::Environment(format!("could not sign out: {error}")))?;
        let said = if removed {
            "signed out"
        } else {
            "no saved sign-in"
        };
        error::print(&format!("{}: {said}\n", provider.name()))?;
        if provider.key_in_env() {
            error::note("Note: OPENROUTER_API_KEY is still set; unset it to sign out fully.\n");
        }
    }
    Ok(exit::OK)
}

/// A key as it may be shown: `sk-or-...abcd`, never more of it.
pub fn mask(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 16 {
        return "...".into();
    }
    let start: String = chars[..6].iter().collect();
    let end: String = chars[chars.len() - 4..].iter().collect();
    format!("{start}...{end}")
}

#[cfg(test)]
#[path = "signin_tests.rs"]
mod tests;
