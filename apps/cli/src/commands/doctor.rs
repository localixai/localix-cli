//! `llx doctor`: is everything a run needs in place? The key, both models (each asked one tiny
//! question), the WebView the browser tool opens, and the shell `bash` runs in. A missing display
//! only leaves the browser out, so it warns without failing.

use std::collections::BTreeMap;
use std::process::{Command, Stdio};

use contracts::chat::{Chat, Message, Request};
use contracts::model::{Decider, Question};
use openrouter::auth::{self, KeySource};
use openrouter::chat::ChatModel;
use openrouter::client::OpenRouter;
use openrouter::system_one::SystemOne;
use serde_json::json;

use crate::cli::ModelOptions;
use crate::commands::login::mask;
use crate::compose::APP;
use crate::error::{self, Failure};
use crate::exit;

/// One check: passed, failed, or a warning that does not fail the run.
enum Mark {
    Pass,
    Fail,
    Warn,
}

fn line(mark: Mark, text: &str) -> Result<(), Failure> {
    let mark = match mark {
        Mark::Pass => "✓",
        Mark::Fail => "✗",
        Mark::Warn => "!",
    };
    error::print(&format!("{mark} {text}\n"))
}

pub fn doctor(models: &ModelOptions) -> Result<i32, Failure> {
    let mut ok = shell()?;
    if tools::browser::has_display() {
        line(
            Mark::Pass,
            "browser: the system WebView (a window opens on first use)",
        )?;
    } else {
        line(
            Mark::Warn,
            "browser: no display, so the browser tool is left out (run under xvfb-run for it)",
        )?;
    }
    let Some((key, source)) = auth::key(APP) else {
        line(
            Mark::Fail,
            "no OpenRouter key: set OPENROUTER_API_KEY or run `llx login`",
        )?;
        return Ok(exit::ENVIRONMENT);
    };
    match source {
        KeySource::Env => line(
            Mark::Pass,
            &format!("key {} from OPENROUTER_API_KEY", mask(&key)),
        )?,
        KeySource::Saved(path) => line(
            Mark::Pass,
            &format!("key {} saved in {}", mask(&key), path.display()),
        )?,
    }
    let client = OpenRouter::new(key, APP);
    ok &= agent_model(&client, &models.model)?;
    ok &= decision_model(&client, &models.decision_model)?;
    Ok(if ok { exit::OK } else { exit::ENVIRONMENT })
}

/// The shell `bash` runs commands with, as the tool names it.
fn shell() -> Result<bool, Failure> {
    let name = tools::shell::SHELL;
    let (program, flag) = name.split_once(' ').unwrap_or((name, ""));
    let ran = Command::new(program)
        .args([flag, "echo ok"])
        .stdin(Stdio::null())
        .output();
    match ran {
        Ok(output) if output.status.success() => {
            line(Mark::Pass, &format!("shell: {name}"))?;
            Ok(true)
        }
        Ok(output) => {
            line(
                Mark::Fail,
                &format!("shell: {name} exited {}", output.status),
            )?;
            Ok(false)
        }
        Err(error) => {
            line(Mark::Fail, &format!("shell: {name} did not start: {error}"))?;
            Ok(false)
        }
    }
}

fn agent_model(client: &OpenRouter, model: &str) -> Result<bool, Failure> {
    let chat = ChatModel {
        client,
        model: model.to_string(),
    };
    let hello = [Message::User("Reply with the word ready.".into())];
    let request = Request {
        conversation: &hello,
        tools: &[],
        may_call: false,
    };
    match chat.complete(request) {
        Ok(completion) => {
            let text = format!(
                "agent model {} answers in {} ms",
                completion.usage.model, completion.usage.millis
            );
            line(Mark::Pass, &text)?;
            Ok(true)
        }
        Err(error) => {
            line(Mark::Fail, &format!("agent model {model}: {error}"))?;
            Ok(false)
        }
    }
}

fn decision_model(client: &OpenRouter, model: &str) -> Result<bool, Failure> {
    let decider = SystemOne {
        client,
        model: model.to_string(),
    };
    let question = Question::Noul {
        instructions: json!("Is the sky in the state blue?"),
    };
    let questions = BTreeMap::from([("q".to_string(), question)]);
    match decider.decide(&json!({"sky": "blue"}), &questions) {
        Ok(decision) => {
            let text = format!(
                "decision model {} answers in {} ms",
                decision.model, decision.millis
            );
            line(Mark::Pass, &text)?;
            Ok(true)
        }
        Err(error) => {
            line(Mark::Fail, &format!("decision model {model}: {error}"))?;
            Ok(false)
        }
    }
}
