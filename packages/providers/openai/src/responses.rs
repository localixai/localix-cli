//! The chat port over the Responses API of the ChatGPT backend, signed in with ChatGPT. The
//! backend streams every reply and stores nothing (`stream`, `store: false`), so each request
//! carries the whole conversation; the standing instructions go first as a developer message.
//! The access token is refreshed before it lapses, and once more when a request is refused.

use std::sync::{Condvar, Mutex};

use contracts::catalog::Selection;
use contracts::chat::{Chat, ChatError, Completion, Request, ToolCall, Usage};
use provider_kit::http::{Http, HttpError};
use provider_kit::retry::{Attempt, PAUSE, TRANSIENT, retried};
use provider_kit::{App, sse};
use serde_json::{Value, json};

use crate::auth;
use crate::tokens::{self, Tokens};

mod request;
mod route;
use request::body;

pub(crate) const BASE: &str = "https://chatgpt.com/backend-api/codex";

/// Where refreshed tokens are kept, and read back from when another process refreshed first.
pub trait Keep: Send + Sync {
    fn load(&self) -> Option<Tokens>;
    fn save(&self, tokens: &Tokens);
}

/// The tokens file of `App`.
pub struct Saved(pub App);

impl Keep for Saved {
    fn load(&self) -> Option<Tokens> {
        tokens::load(self.0)
    }

    fn save(&self, tokens: &Tokens) {
        let _ = tokens::save(self.0, tokens);
    }
}

/// The tokens in use, and whether a thread is refreshing them.
struct Held {
    tokens: Tokens,
    refreshing: bool,
}

pub struct ChatModel {
    http: Box<dyn Http>,
    keep: Box<dyn Keep>,
    app: App,
    selection: Selection,
    tokens: Mutex<Held>,
    /// Rung when a refresh ends.
    renewed: Condvar,
    base: String,
    /// Each task's machine, as the backend named it.
    routes: route::Routes,
}

impl ChatModel {
    pub fn new(
        app: App,
        http: Box<dyn Http>,
        keep: Box<dyn Keep>,
        tokens: Tokens,
        selection: Selection,
    ) -> Self {
        Self {
            http,
            keep,
            app,
            selection,
            tokens: Mutex::new(Held {
                tokens,
                refreshing: false,
            }),
            renewed: Condvar::new(),
            base: BASE.into(),
            routes: route::Routes::default(),
        }
    }

    /// The tokens to send, refreshed first when they are about to lapse (or `force`d). One thread
    /// refreshes while the others wait on the condvar: the lock is never held across the
    /// request (R6), and no two refresh at once (R7).
    fn tokens(&self, force: bool) -> Result<Tokens, ChatError> {
        let mut state = self.tokens.lock().unwrap_or_else(|p| p.into_inner());
        let mut waited = false;
        while state.refreshing {
            state = self.renewed.wait(state).unwrap_or_else(|p| p.into_inner());
            waited = true;
        }
        let now = tokens::now();
        // A refresh that ended while this thread waited did what it came to do.
        if (waited || !force) && !state.tokens.stale(now) {
            return Ok(state.tokens.clone());
        }
        state.refreshing = true;
        let held = state.tokens.clone();
        drop(state);
        let outcome = self.renew(&held, now);
        let mut state = self.tokens.lock().unwrap_or_else(|p| p.into_inner());
        state.refreshing = false;
        if let Ok(fresh) = &outcome {
            state.tokens.clone_from(fresh);
        }
        drop(state);
        self.renewed.notify_all();
        outcome
    }

    /// Fresh tokens for `held`: another llx's, when it refreshed first (ours were spent), else
    /// the issuer's, kept before they are used.
    fn renew(&self, held: &Tokens, now: u64) -> Result<Tokens, ChatError> {
        if let Some(kept) = self
            .keep
            .load()
            .filter(|kept| kept != held && !kept.stale(now))
        {
            return Ok(kept);
        }
        let fresh = auth::refresh(self.http.as_ref(), held, now).map_err(ChatError::Unavailable)?;
        self.keep.save(&fresh);
        Ok(fresh)
    }
}

impl Chat for ChatModel {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        let task = route::task(request.conversation);
        let session = route::session(request.conversation);
        let body = body(&self.selection, request).to_string();
        let address = format!("{}/responses", self.base);
        let started = std::time::Instant::now();
        let mut renewed = false;
        retried(PAUSE, || {
            let tokens = match self.tokens(false) {
                Ok(tokens) => tokens,
                Err(error) => return Attempt::Done(Err(error)),
            };
            let mut headers = vec![
                ("Authorization", format!("Bearer {}", tokens.access)),
                ("chatgpt-account-id", tokens.account),
                ("originator", self.app.slug.to_string()),
                ("session-id", session.clone()),
                ("Content-Type", "application/json".into()),
                ("Accept", "text/event-stream".into()),
            ];
            if let Some(token) = self.routes.token(&task) {
                headers.push((route::HEADER, token));
            }
            let headers: Vec<(&str, &str)> =
                headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let reply = match self.http.post(&address, &headers, &body) {
                Ok(reply) => reply,
                Err(HttpError::Unsent(why)) => {
                    return Attempt::Again(ChatError::Unavailable(format!(
                        "ChatGPT is unreachable: {why}"
                    )));
                }
                // A reply being written is billed: asking again would pay for it twice.
                Err(HttpError::Unanswered(why)) => {
                    return Attempt::Done(Err(ChatError::Unavailable(format!(
                        "ChatGPT did not answer: {why}"
                    ))));
                }
            };
            match reply.status {
                200 => Attempt::Done({
                    self.routes.keep(task.clone(), &reply);
                    read(&reply.body, self.selection.model().id.as_str()).map(|mut completion| {
                        completion.usage.millis =
                            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                        completion
                    })
                }),
                401 if !renewed => {
                    renewed = true;
                    match self.tokens(true) {
                        Ok(_) => Attempt::Again(ChatError::Unavailable(refusal(401, &reply.body))),
                        Err(error) => Attempt::Done(Err(error)),
                    }
                }
                // A 429 here is the plan's usage limit, hours away: not worth a second try.
                status if TRANSIENT.contains(&status) => {
                    Attempt::Again(ChatError::Unavailable(refusal(status, &reply.body)))
                }
                status => Attempt::Done(Err(ChatError::Unavailable(refusal(status, &reply.body)))),
            }
        })
    }
}

/// The completion in a streamed reply: its finished output items and the usage it ended with.
pub(crate) fn read(body: &str, model: &str) -> Result<Completion, ChatError> {
    let mut completion = Completion::default();
    let mut said: Vec<String> = vec![];
    let mut ended = false;
    for event in sse::events(body) {
        match event["type"].as_str().unwrap_or("") {
            "response.output_item.done" => {
                let item = &event["item"];
                match item["type"].as_str() {
                    Some("message") => said.extend(
                        item["content"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter(|part| part["type"] == "output_text")
                            .filter_map(|part| part["text"].as_str().map(str::to_string)),
                    ),
                    Some("function_call") => completion.calls.extend(call(item)),
                    // Nothing is stored at the backend: only a sealed reasoning can go back, and
                    // only to the model that sealed it.
                    Some("reasoning") if item["encrypted_content"].is_string() => {
                        completion
                            .carried
                            .push(json!({"model": model, "item": item}));
                    }
                    _ => {}
                }
            }
            "response.completed" | "response.incomplete" => {
                completion.usage = usage(&event["response"], model);
                ended = true;
            }
            "response.failed" | "error" => {
                let error = event.pointer("/response/error").unwrap_or(&event);
                let message = error["message"].as_str().unwrap_or("no message");
                return Err(ChatError::Unavailable(format!("ChatGPT failed: {message}")));
            }
            _ => {}
        }
    }
    if !ended {
        return Err(ChatError::Invalid(
            "the reply ended before it was complete".into(),
        ));
    }
    let said = said.join("\n");
    completion.text = Some(said.trim().to_string()).filter(|text| !text.is_empty());
    Ok(completion)
}

fn call(item: &Value) -> Option<ToolCall> {
    let raw = item["arguments"].as_str().unwrap_or("{}");
    Some(ToolCall {
        id: item["call_id"].as_str()?.to_string(),
        name: item["name"].as_str()?.to_string(),
        // Arguments that are not JSON stay as their text: the agent tells the model they were
        // incomplete instead of running the tool with none.
        arguments: serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string())),
    })
}

/// What the reply took. A ChatGPT plan is paid by subscription: no cost per call.
fn usage(response: &Value, model: &str) -> Usage {
    let tokens = |pointer: &str| {
        response
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    Usage {
        model: response["model"].as_str().unwrap_or(model).to_string(),
        cost: 0.0,
        millis: 0,
        input_tokens: tokens("/usage/input_tokens"),
        cached_tokens: tokens("/usage/input_tokens_details/cached_tokens"),
        output_tokens: tokens("/usage/output_tokens"),
    }
}

/// Why the backend refused, in words: its own message where it sent one.
fn refusal(status: u16, body: &str) -> String {
    let value: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let message = value
        .pointer("/error/message")
        .or_else(|| value.get("detail"))
        .and_then(Value::as_str)
        .map_or_else(|| body.chars().take(300).collect(), str::to_string);
    match status {
        429 => format!("the ChatGPT plan's usage limit is reached: {message}"),
        401 | 403 => format!("ChatGPT refused the sign-in ({status}): {message}; /login again"),
        _ => format!("ChatGPT answered {status}: {message}"),
    }
}

#[cfg(test)]
#[path = "responses_tests.rs"]
mod tests;
