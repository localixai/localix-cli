//! `llx doctor`: is everything a run needs in place? The sign-in to each provider, both models
//! (each asked one tiny question), the WebView the browser tool opens, and the shell `bash` runs
//! in. A missing display only leaves the browser out, and a missing decision model only its
//! actions, so both warn without failing.

use std::collections::BTreeMap;
use std::process::{Command, Stdio};

use contracts::chat::{Chat, Message, Request};
use contracts::model::{Decider, Question};
use serde_json::json;

use crate::choice;
use crate::cli::ModelOptions;
use crate::error::{self, Failure};
use crate::exit;
use crate::live::NO_DECIDER;
use crate::providers::{self, Provider};

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
    let choice = choice::resolve(models, choice::saved())?;
    // The provider in use, and every other one signed in to: the catalog lists hundreds.
    let listed = Provider::all()
        .into_iter()
        .filter(|p| *p == choice.provider || p.signed_in().is_some());
    for provider in listed {
        match provider.signed_in() {
            Some(from) => line(
                Mark::Pass,
                &format!("{}: signed in with {from}", provider.name()),
            )?,
            None => line(
                if provider == choice.provider {
                    Mark::Fail
                } else {
                    Mark::Warn
                },
                &format!(
                    "{}: not signed in (type /login in a session)",
                    provider.name()
                ),
            )?,
        }
    }
    match choice.provider.chat(&choice.selection) {
        Some(chat) => ok &= agent_model(chat.as_ref(), &choice.label())?,
        None => ok = false,
    }
    match providers::decider(&models.decision_model) {
        Some(decider) => ok &= decision_model(decider.as_ref(), &models.decision_model)?,
        None => line(Mark::Warn, &format!("decision model: {NO_DECIDER}"))?,
    }
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

fn agent_model(chat: &dyn Chat, model: &str) -> Result<bool, Failure> {
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

fn decision_model(decider: &dyn Decider, model: &str) -> Result<bool, Failure> {
    let question = Question::Noul {
        instructions: json!("Is the sky in the state blue?"),
    };
    let questions = BTreeMap::from([("q".to_string(), question)]);
    match decider.decide(&json!({"sky": "blue"}), &questions) {
        // A reply with no answer in it is no working model, whatever its status.
        Ok(decision) if !decision.answers.contains_key("q") => {
            line(Mark::Fail, &format!("decision model {model}: no answer"))?;
            Ok(false)
        }
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
