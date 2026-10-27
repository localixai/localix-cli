//! Signing in to a provider, for `/login`, and `llx logout`, which forgets every saved sign-in.
//! Keys and tokens are never shown: a key is read with the echo off, and only its masked ends
//! are printed.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use provider_kit::http::Web;
use provider_kit::oauth;

use crate::compose::APP;
use crate::error::{self, Failure};
use crate::exit;
use crate::output::lock;
use crate::picker::Terminal;
use crate::providers::{self, Method, Provider, Vars};

/// How long the token endpoint may take.
const TOKEN_TIMEOUT: Duration = Duration::from_secs(60);

/// Sign in to `provider` with `method`; what was saved, in words with no secret in them.
pub(crate) fn sign_in(
    provider: &Provider,
    method: Method,
    terminal: &mut dyn Terminal,
    say: &dyn Fn(&str),
    vars: Vars,
) -> Result<String, String> {
    let saved = |path: PathBuf| format!("saved in {}", path.display());
    match (provider.id(), method) {
        ("openai", Method::Browser) => {
            let login = openai::auth::Login::start(APP)?;
            let code = browser(login.address(), terminal, say, &|t| login.wait(t), &|p| {
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
        ("openrouter", Method::Browser) => {
            let login = openrouter::auth::Login::start(APP)?;
            let code = browser(login.address(), terminal, say, &|t| login.wait(t), &|p| {
                login.pasted(p)
            })?;
            let key = login.finish(&providers::openrouter_client(String::new(), vars), &code)?;
            let path = openrouter::auth::save(APP, &key).map_err(|e| e.to_string())?;
            Ok(format!(
                "Signed in: the key {} is {}.",
                mask(&key),
                saved(path)
            ))
        }
        (_, _) => {
            let prompt = format!(
                "Paste the {} key and press Enter (it is not shown): ",
                provider.name()
            );
            let line = terminal.line(&prompt, true).ok_or("no key was pasted")?;
            let key = openrouter::auth::as_key(&line).ok_or("that is not a key")?;
            let path = provider.save_key(key).map_err(|e| e.to_string())?;
            let mut said = format!("Saved the key {}; {}.", mask(key), saved(path));
            if provider.key_in_env(vars) {
                said.push_str(" Its variable is set, and it wins over the saved key.");
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
/// on another machine); the authorization code. The line for the address closes by itself once
/// the browser is back: nothing to press. It is read on this thread only, so
/// nothing is left reading stdin once the sign-in ends.
pub(crate) fn come_back(
    terminal: &mut dyn Terminal,
    say: &dyn Fn(&str),
    wait: &dyn Fn(Duration) -> Option<Result<String, String>>,
    pasted: &dyn Fn(&str) -> Result<String, String>,
) -> Result<String, String> {
    let prompt = "Finish signing in in the browser; llx goes on by itself. If the browser is \
                  on another machine, paste the address it ended on (Enter checks again, \
                  q cancels) > ";
    let back = Mutex::new(None);
    let done = || {
        let mut back = lock(&back);
        if back.is_none() {
            *back = wait(Duration::ZERO);
        }
        back.is_some()
    };
    loop {
        let line = terminal.line_until(prompt, &done);
        let came = lock(&back).take();
        if let Some(outcome) = came {
            return outcome;
        }
        let line = line.ok_or("cancelled")?;
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
pub(crate) fn logout() -> Result<i32, Failure> {
    let mut any = false;
    let mut in_env = vec![];
    for provider in Provider::all() {
        let removed = provider
            .forget()
            .map_err(|error| Failure::Environment(format!("could not sign out: {error}")))?;
        if removed {
            any = true;
            error::print(&format!("{}: signed out\n", provider.name()))?;
        }
        if provider.key_in_env(None) {
            in_env.push(provider.name());
        }
    }
    if !any {
        error::print("No saved sign-in.\n")?;
    }
    // After what stdout says: a closed one has ended the command quietly by now.
    for name in in_env {
        error::note(&format!(
            "Note: {name}'s key is still in the environment; unset it to sign out fully.\n"
        ));
    }
    Ok(exit::OK)
}

/// A key as it may be shown: `sk-or-...abcd`, never more of it.
pub(crate) fn mask(key: &str) -> String {
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
