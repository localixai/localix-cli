//! The chat port over OpenRouter's chat completions, with tool calling: any of its chat models.

use contracts::chat::{Chat, ChatError, Completion, Message, Request, ToolCall, Usage};
use contracts::tool::ToolSpec;
use serde_json::{Value, json};

use crate::client::{ApiError, OpenRouter};

pub const DEFAULT_MODEL: &str = "anthropic/claude-haiku-5.5";
/// Enough for a short sub-goal with its values, or a final answer.
const MAX_TOKENS: u32 = 1500;

pub struct ChatModel<'a> {
    pub client: &'a OpenRouter,
    pub model: String,
}

impl Chat for ChatModel<'_> {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        let mut body = json!({
            "model": self.model,
            "messages": cached(request.conversation.iter().map(wire).collect()),
            "max_tokens": MAX_TOKENS,
            // Thinking costs seconds per turn; the agent reasons in its tool calls instead.
            "reasoning": {"enabled": false},
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
                model: value["model"].as_str().unwrap_or(&self.model).to_string(),
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
    }
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
        }
    };
    if let Some(system) = messages.first_mut().filter(|m| m["role"] == "system") {
        mark(system);
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
        arguments: serde_json::from_str(raw).unwrap_or_else(|_| json!({})),
    })
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
