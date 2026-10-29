//! `llx doctor`: is everything a run needs in place? The sign-in to each provider, both models
//! (each asked one tiny question), and the shell `bash` runs in. A missing decision model only
//! leaves `llx decide` out, so it warns without failing.

use std::collections::BTreeMap;
use std::process::Stdio;

use contracts::chat::{Chat, Message, Request};
use contracts::model::{Decider, Question};
use serde_json::json;

use crate::choice;
use crate::cli::ModelOptions;
use crate::error::{self, Failure};
use crate::exit;
use crate::providers::{self, Provider};

/// One check: passed, failed, or a warning that does not fail the run.
#[derive(Clone, Copy)]
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

pub(crate) fn doctor(models: &ModelOptions) -> Result<i32, Failure> {
    let mut ok = report(shell())?;
    let choice = choice::resolve(models, choice::saved(), None)?;
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
        Some(chat) => ok &= report(agent_model(chat.as_ref(), &choice.label()))?,
        None => ok = false,
    }
    match providers::decider(&models.decision_model) {
        Some(decider) => ok &= report(decision_model(decider.as_ref(), &models.decision_model))?,
        None => line(
            Mark::Warn,
            "decision model: `llx decide` runs on OpenRouter: set OPENROUTER_API_KEY",
        )?,
    }
    Ok(if ok { exit::OK } else { exit::ENVIRONMENT })
}

/// A check's mark and its line, printed; whether it passed (a warning does not fail the run).
fn report((mark, text): (Mark, String)) -> Result<bool, Failure> {
    line(mark, &text)?;
    Ok(!matches!(mark, Mark::Fail))
}

/// The shell `bash` runs commands with, as the tool names it.
#[expect(
    clippy::disallowed_methods,
    reason = "R10 debt: `echo ok` ends at once, with no deadline or stop yet"
)]
fn shell() -> (Mark, String) {
    let name = tools::shell::SHELL;
    match tools::shell::shell("echo ok").stdin(Stdio::null()).output() {
        Ok(output) if output.status.success() => (Mark::Pass, format!("shell: {name}")),
        Ok(output) => (
            Mark::Fail,
            format!("shell: {name} exited {}", output.status),
        ),
        Err(error) => (Mark::Fail, format!("shell: {name} did not start: {error}")),
    }
}

fn agent_model(chat: &dyn Chat, model: &str) -> (Mark, String) {
    let hello = [Message::User("Reply with the word ready.".into())];
    let request = Request {
        conversation: &hello,
        tools: &[],
        may_call: false,
        deadline: None,
    };
    match chat.complete(request) {
        Ok(completion) => (
            Mark::Pass,
            format!(
                "agent model {} answers in {} ms",
                completion.usage.model, completion.usage.millis
            ),
        ),
        Err(error) => (Mark::Fail, format!("agent model {model}: {error}")),
    }
}

fn decision_model(decider: &dyn Decider, model: &str) -> (Mark, String) {
    let question = Question::Noul {
        instructions: json!("Is the sky in the state blue?"),
    };
    let questions = BTreeMap::from([("q".to_string(), question)]);
    match decider.decide(&json!({"sky": "blue"}), &questions) {
        // A reply with no answer in it is no working model, whatever its status.
        Ok(decision) if !decision.answers.contains_key("q") => {
            (Mark::Fail, format!("decision model {model}: no answer"))
        }
        Ok(decision) => (
            Mark::Pass,
            format!(
                "decision model {} answers in {} ms",
                decision.model, decision.millis
            ),
        ),
        Err(error) => (Mark::Fail, format!("decision model {model}: {error}")),
    }
}
