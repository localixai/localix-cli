//! `lxsurf doctor` and `lxsurf models`: is everything a run needs in place?

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::auth::{self, KeySource};
use crate::decision::{self, Decider, Question, SystemOne};
use crate::exit;
use crate::openrouter::OpenRouter;
use crate::planner::{self, ChatModel, Planner};

fn client() -> Result<OpenRouter, i32> {
    match auth::key() {
        Some((key, _)) => Ok(OpenRouter::new(key)),
        None => {
            eprintln!("✗ no OpenRouter key: set OPENROUTER_API_KEY or run `lxsurf login`");
            Err(exit::ENVIRONMENT)
        }
    }
}

pub fn doctor() -> i32 {
    let mut ok = true;
    match auth::key() {
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
    let decision_model =
        std::env::var("LXSURF_DECISION_MODEL").unwrap_or_else(|_| decision::DEFAULT_MODEL.into());
    let decider = SystemOne {
        client: &client,
        model: decision_model.clone(),
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
            println!("✗ decision model {decision_model}: {error}");
        }
    }
    let model = std::env::var("LXSURF_MODEL").unwrap_or_else(|_| planner::DEFAULT_MODEL.into());
    let chat = ChatModel {
        client: &client,
        model: model.clone(),
    };
    let messages = [json!({"role": "user", "content": "Reply with OK."})];
    match chat.turn(&messages, &json!([])) {
        Ok(reply) => println!("✓ chat model {model} answers in {} ms", reply.millis),
        Err(error) => {
            ok = false;
            println!("✗ chat model {model}: {error}");
        }
    }
    if cfg!(target_os = "linux")
        && std::env::var_os("DISPLAY").is_none()
        && std::env::var_os("WAYLAND_DISPLAY").is_none()
    {
        ok = false;
        println!("✗ no display: run under xvfb-run (WebKitGTK needs an X server)");
    } else {
        println!("✓ system WebView");
    }
    if ok { exit::OK } else { exit::ENVIRONMENT }
}

pub fn models(decision: bool) -> i32 {
    let client = match client() {
        Ok(client) => client,
        Err(code) => return code,
    };
    let listing = match client.get("/models") {
        Ok(listing) => listing,
        Err(error) => {
            eprintln!("lxsurf: {error}");
            return exit::ENVIRONMENT;
        }
    };
    let models = listing
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if decision {
        // System One models are not chat models; the public listing names their router at most.
        println!("{}  (default)", decision::DEFAULT_MODEL);
        for model in &models {
            let id = model.get("id").and_then(Value::as_str).unwrap_or("");
            if id.starts_with("typesafe/") || id.starts_with("~typesafe/") {
                println!("{id}");
            }
        }
        return exit::OK;
    }
    for model in &models {
        let tools = model
            .get("supported_parameters")
            .and_then(Value::as_array)
            .is_some_and(|params| params.iter().any(|p| p == "tools"));
        if tools {
            let id = model.get("id").and_then(Value::as_str).unwrap_or("");
            let default = if id == planner::DEFAULT_MODEL {
                "  (default)"
            } else {
                ""
            };
            println!("{id}{default}");
        }
    }
    exit::OK
}
