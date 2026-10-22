//! The chat port over OpenRouter's chat completions, with tool calling: any of its chat models.

use contracts::catalog::{Effort, Selection};
use contracts::chat::{Chat, ChatError, Completion, Message, Request, ToolCall, Usage};
use contracts::tool::{Image, ToolSpec};
use serde_json::{Value, json};

use crate::client::{ApiError, OpenRouter};

pub struct ChatModel {
    pub client: OpenRouter,
    pub selection: Selection,
}

impl Chat for ChatModel {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        let model = &self.selection.model().id;
        let mut body = json!({
            "model": model,
            "messages": cached(request.conversation.iter().map(wire).collect()),
            // No output limit: a whole file in one call is a long reply, and a cut one breaks the
            // call's arguments. The model's own maximum applies, as in Codex.
            "reasoning": reasoning(self.selection.effort()),
            "usage": {"include": true},
        });
        if !request.tools.is_empty() {
            body["tools"] = request.tools.iter().map(function).collect();
            body["tool_choice"] = json!(if request.may_call { "auto" } else { "none" });
            // One call at a time, so every call sees what the previous one left.
            body["parallel_tool_calls"] = json!(false);
        }
        let (value, timing) = self
            .client
            .post("/chat/completions", &body)
            .map_err(|error| match error {
                ApiError::Invalid(message) => ChatError::Invalid(message),
                other => ChatError::Unavailable(other.to_string()),
            })?;
        let message = value
            .pointer("/choices/0/message")
            .ok_or_else(|| ChatError::Invalid("no message in the completion".into()))?;
        let calls = message
            .get("tool_calls")
            .and_then(Value::as_array)
            .map(|calls| calls.iter().filter_map(read_call).collect())
            .unwrap_or_default();
        let text = message
            .get("content")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(String::from);
        let tokens = |pointer: &str| value.pointer(pointer).and_then(Value::as_u64).unwrap_or(0);
        Ok(Completion {
            text,
            calls,
            usage: Usage {
                model: value["model"].as_str().unwrap_or(model).to_string(),
                cost: value
                    .pointer("/usage/cost")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
                millis: timing.millis,
                input_tokens: tokens("/usage/prompt_tokens"),
                cached_tokens: tokens("/usage/prompt_tokens_details/cached_tokens"),
                output_tokens: tokens("/usage/completion_tokens"),
            },
        })
    }
}

/// The level as OpenRouter's `reasoning` takes it. Off is no thinking at all: it costs seconds
/// per turn, and the agent reasons in its tool calls instead.
pub fn reasoning(effort: Effort) -> Value {
    match effort {
        Effort::Off => json!({"enabled": false}),
        effort => json!({"effort": effort.name()}),
    }
}

/// A message in the chat-completions wire format.
fn wire(message: &Message) -> Value {
    match message {
        Message::System(text) => json!({"role": "system", "content": text}),
        Message::User(text) => json!({"role": "user", "content": text}),
        Message::Assistant { text, calls } => {
            let mut out = json!({"role": "assistant", "content": text});
            if !calls.is_empty() {
                out["tool_calls"] = calls
                    .iter()
                    .map(|call| {
                        json!({"id": call.id, "type": "function", "function": {
                            "name": call.name, "arguments": call.arguments.to_string()}})
                    })
                    .collect();
            }
            out
        }
        // The wire has no failure flag for a tool result: the text says it.
        Message::ToolResult {
            id,
            content,
            failed,
        } => {
            let content = if *failed {
                format!("Error: {content}")
            } else {
                content.clone()
            };
            json!({"role": "tool", "tool_call_id": id, "content": content})
        }
        Message::Image { caption, image } => picture(caption, image),
    }
}

/// A picture as a user message with a text part and an image part: a tool message carries text
/// only, so it follows the call's result instead.
fn picture(caption: &str, image: &Image) -> Value {
    let url = format!("data:{};base64,{}", image.media_type, image.data);
    json!({"role": "user", "content": [
        {"type": "text", "text": caption},
        {"type": "image_url", "image_url": {"url": url}},
    ]})
}

fn function(spec: &ToolSpec) -> Value {
    json!({"type": "function", "function": {
        "name": spec.name, "description": spec.description, "parameters": spec.parameters}})
}

/// The conversation with prompt-cache breakpoints on the system prompt and the newest message, so
/// each turn re-reads only what was added since the last one (Anthropic models through
/// OpenRouter; other providers ignore the marks or cache on their own).
fn cached(mut messages: Vec<Value>) -> Vec<Value> {
    let mark = |message: &mut Value| {
        if let Some(text) = message["content"].as_str().map(str::to_string) {
            message["content"] =
                json!([{"type": "text", "text": text, "cache_control": {"type": "ephemeral"}}]);
        } else if let Some(last) = message["content"].as_array_mut().and_then(|p| p.last_mut()) {
            // Already in parts (a picture): the mark goes on the last one.
            last["cache_control"] = json!({"type": "ephemeral"});
        }
    };
    if let Some(system) = messages.first_mut().filter(|m| m["role"] == "system") {
        mark(system);
    }
    // The message after the system prompt is the context (the environment, the instructions):
    // the same for every task in a workspace, so a new session reads it from the cache too.
    if messages.len() > 2
        && messages[0]["role"] == "system"
        && let Some(context) = messages.get_mut(1).filter(|m| m["role"] == "user")
    {
        mark(context);
    }
    if messages.len() > 1
        && let Some(last) = messages.last_mut()
    {
        mark(last);
    }
    messages
}

fn read_call(call: &Value) -> Option<ToolCall> {
    let function = call.get("function")?;
    let raw = function["arguments"].as_str().unwrap_or("{}");
    Some(ToolCall {
        id: call.get("id")?.as_str()?.to_string(),
        name: function.get("name")?.as_str()?.to_string(),
        // Arguments that are not JSON stay as their text: the agent tells the model they were
        // incomplete instead of running the tool with none.
        arguments: serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string())),
    })
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
