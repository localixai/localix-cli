//! The chat port over the Responses API of the ChatGPT backend, signed in with ChatGPT. The
//! backend streams every reply and stores nothing (`stream`, `store: false`), so each request
//! carries the whole conversation; the standing instructions go first as a developer message.
//! The access token is refreshed before it lapses, and once more when a request is refused.

use std::sync::Mutex;
use std::time::Duration;

use contracts::catalog::{Effort, Selection};
use contracts::chat::{Chat, ChatError, Completion, Message, Request, ToolCall, Usage};
use contracts::tool::ToolSpec;
use provider_kit::http::Http;
use provider_kit::{App, oauth, sse};
use serde_json::{Value, json};

use crate::auth;
use crate::tokens::{self, Tokens};

pub const BASE: &str = "https://chatgpt.com/backend-api/codex";
const ATTEMPTS: u32 = 3;
const RETRY_STATUSES: [u16; 4] = [500, 502, 503, 504];

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

pub struct ChatModel {
    http: Box<dyn Http>,
    keep: Box<dyn Keep>,
    app: App,
    selection: Selection,
    tokens: Mutex<Tokens>,
    /// One per process: the backend keys its prompt cache on it.
    session: String,
    pub base: String,
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
            tokens: Mutex::new(tokens),
            // Only the prompt cache's key: without randomness, the time is unique enough.
            session: oauth::random(16).unwrap_or_else(|_| format!("llx-{}", tokens::now())),
            base: BASE.into(),
        }
    }

    /// The tokens to send, refreshed first when they are about to lapse (or `force`d).
    fn tokens(&self, force: bool) -> Result<Tokens, ChatError> {
        let mut held = self
            .tokens
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let now = tokens::now();
        if !force && !held.stale(now) {
            return Ok(held.clone());
        }
        // Another llx may have refreshed them already: its tokens are good, and ours were spent.
        if let Some(kept) = self
            .keep
            .load()
            .filter(|kept| *kept != *held && !kept.stale(now))
        {
            *held = kept;
            return Ok(held.clone());
        }
        let fresh =
            auth::refresh(self.http.as_ref(), &held, now).map_err(ChatError::Unavailable)?;
        self.keep.save(&fresh);
        *held = fresh;
        Ok(held.clone())
    }
}

impl Chat for ChatModel {
    #[expect(
        clippy::disallowed_methods,
        reason = "a backoff between attempts, which no event ends sooner"
    )]
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        let body = body(&self.selection, &self.session, request).to_string();
        let address = format!("{}/responses", self.base);
        let started = std::time::Instant::now();
        let (mut renewed, mut last) = (false, ChatError::Unavailable("no attempt".into()));
        for attempt in 0..ATTEMPTS {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(400 << attempt));
            }
            let tokens = self.tokens(false)?;
            let headers = [
                ("Authorization", format!("Bearer {}", tokens.access)),
                ("chatgpt-account-id", tokens.account.clone()),
                ("originator", self.app.slug.to_string()),
                ("session_id", self.session.clone()),
                ("Content-Type", "application/json".into()),
                ("Accept", "text/event-stream".into()),
            ];
            let headers: Vec<(&str, &str)> =
                headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let reply = match self.http.post(&address, &headers, &body) {
                Ok(reply) => reply,
                Err(error) => {
                    last = ChatError::Unavailable(format!("ChatGPT is unreachable: {error}"));
                    continue;
                }
            };
            match reply.status {
                200 => {
                    let mut completion = read(&reply.body, self.selection.model().id.as_str())?;
                    completion.usage.millis =
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                    return Ok(completion);
                }
                401 if !renewed => {
                    renewed = true;
                    self.tokens(true)?;
                }
                status => {
                    last = ChatError::Unavailable(refusal(status, &reply.body));
                    if !RETRY_STATUSES.contains(&status) {
                        return Err(last);
                    }
                }
            }
        }
        Err(last)
    }
}

/// The request body for `request`.
pub fn body(selection: &Selection, session: &str, request: Request) -> Value {
    let mut body = json!({
        "model": selection.model().id,
        "input": request.conversation.iter().flat_map(items).collect::<Vec<_>>(),
        "store": false,
        "stream": true,
        "include": [],
        "prompt_cache_key": session,
    });
    // Off is said: a request without the field reasons at the backend's own default.
    if !selection.model().efforts.is_empty() {
        let effort = selection.effort();
        let name = if effort == Effort::Off {
            "none"
        } else {
            effort.name()
        };
        body["reasoning"] = json!({"effort": name});
    }
    if !request.tools.is_empty() {
        body["tools"] = request.tools.iter().map(function).collect();
        body["tool_choice"] = json!(if request.may_call { "auto" } else { "none" });
        // Several calls in one turn: independent reads and searches cost one turn, not many. The
        // agent runs together only the calls whose tools allow it, the rest in order.
        body["parallel_tool_calls"] = json!(true);
    }
    body
}

/// A message as the Responses API's input items.
fn items(message: &Message) -> Vec<Value> {
    let text = |role: &str, kind: &str, text: &str| json!({"type": "message", "role": role, "content": [{"type": kind, "text": text}]});
    match message {
        Message::System(rules) => vec![text("developer", "input_text", rules)],
        Message::User(said) => vec![text("user", "input_text", said)],
        Message::Assistant { text: said, calls } => said
            .iter()
            .map(|said| text("assistant", "output_text", said))
            .chain(calls.iter().map(|call| {
                let arguments = (call.arguments.as_str())
                    .map_or_else(|| call.arguments.to_string(), str::to_string);
                json!({"type": "function_call", "call_id": call.id, "name": call.name,
                    "arguments": arguments})
            }))
            .collect(),
        // A picture goes in as the person's message, after the call that took it.
        Message::Image { caption, image } => vec![json!({"type": "message", "role": "user",
            "content": [{"type": "input_text", "text": caption},
                {"type": "input_image",
                 "image_url": format!("data:{};base64,{}", image.media_type, image.data)}]})],
        // The wire has no failure flag for a tool result: the text says it.
        Message::ToolResult {
            id,
            content,
            failed,
        } => {
            let output = match failed {
                true => format!("Error: {content}"),
                false => content.clone(),
            };
            vec![json!({"type": "function_call_output", "call_id": id, "output": output})]
        }
    }
}

fn function(spec: &ToolSpec) -> Value {
    json!({"type": "function", "name": spec.name, "description": spec.description,
        "parameters": spec.parameters, "strict": false})
}

/// The completion in a streamed reply: its finished output items and the usage it ended with.
pub fn read(body: &str, model: &str) -> Result<Completion, ChatError> {
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
