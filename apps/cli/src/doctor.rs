//! `lxsurf doctor` and `lxsurf models`: is everything a run needs in place?

use std::collections::BTreeMap;

use contracts::chat::{Chat, Message, Request};
use contracts::model::{Decider, Question};
use openrouter::auth::{self, KeySource};
use openrouter::chat::ChatModel;
use openrouter::client::OpenRouter;
use openrouter::system_one::{self, SystemOne};
use serde_json::json;

use crate::{APP, exit};

fn client() -> Result<OpenRouter, i32> {
    match auth::key(APP) {
        Some((key, _)) => Ok(OpenRouter::new(key, APP)),
        None => {
            eprintln!("✗ no OpenRouter key: set OPENROUTER_API_KEY or run `lxsurf login`");
            Err(exit::ENVIRONMENT)
        }
    }
}

pub fn doctor(chat_model: &str, model: &str) -> i32 {
    let mut ok = true;
    match auth::key(APP) {
        Some((_, KeySource::Env)) => println!("✓ key from OPENROUTER_API_KEY"),
        Some((_, KeySource::Saved(path))) => {
            println!("✓ key saved by `lxsurf login` ({})", path.display())
        }
        None => {}
    }
    let client = match client() {
        Ok(client) => client,
        Err(code) => return code,
    };
    let chat = ChatModel {
        client: &client,
        model: chat_model.to_string(),
    };
    let hello = [Message::User("Reply with the word ready.".into())];
    let request = Request {
        conversation: &hello,
        tools: &[],
        may_call: false,
    };
    match chat.complete(request) {
        Ok(completion) => println!(
            "✓ agent model {} answers in {} ms",
            completion.usage.model, completion.usage.millis
        ),
        Err(error) => {
            ok = false;
            println!("✗ agent model {chat_model}: {error}");
        }
    }
    let decider = SystemOne {
        client: &client,
        model: model.to_string(),
    };
    let question = Question::Noul {
        instructions: json!("Is the sky in the state blue?"),
    };
    match decider.decide(
        &json!({"sky": "blue"}),
        &BTreeMap::from([("q".to_string(), question)]),
    ) {
        Ok(decision) => println!(
            "✓ decision model {} answers in {} ms",
            decision.model, decision.millis
        ),
        Err(error) => {
            ok = false;
            println!("✗ decision model {model}: {error}");
        }
    }
    if tools::browser::has_display() {
        println!("✓ system WebView");
    } else {
        ok = false;
        println!("✗ no display: run under xvfb-run (WebKitGTK needs an X server)");
    }
    if ok { exit::OK } else { exit::ENVIRONMENT }
}

/// OpenRouter's decision models, cheapest first, with price and context.
pub fn models() -> i32 {
    let client = match client() {
        Ok(client) => client,
        Err(code) => return code,
    };
    match system_one::models(&client) {
        Ok(models) => {
            for (id, price, context) in models {
                let default = if id == system_one::DEFAULT_MODEL {
                    "  (default)"
                } else {
                    ""
                };
                println!(
                    "{id:<40} ${price:.3}/M in · {}k context{default}",
                    context / 1000
                );
            }
            exit::OK
        }
        Err(error) => {
            eprintln!("lxsurf: {error}");
            exit::ENVIRONMENT
        }
    }
}
