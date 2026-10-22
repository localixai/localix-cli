//! The chat model that plans: OpenRouter chat completions with tool calling. It decides where to go,
//! what short sub-goal to hand the decision model and with which exact values, checks the page, and
//! writes the answer.

use serde_json::{Value, json};

use crate::openrouter::{ApiError, OpenRouter};

pub const DEFAULT_MODEL: &str = "anthropic/claude-haiku-5.5";

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Default)]
pub struct Reply {
    pub text: Option<String>,
    pub calls: Vec<ToolCall>,
    /// The assistant message to append to the conversation as it came.
    pub message: Value,
    pub cost: f64,
    pub millis: u64,
    pub model: String,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub output_tokens: u64,
}

pub trait Planner {
    fn turn(&self, messages: &[Value], tools: &Value) -> Result<Reply, ApiError>;
}

pub struct ChatModel<'a> {
    pub client: &'a OpenRouter,
    pub model: String,
}

impl Planner for ChatModel<'_> {
    fn turn(&self, messages: &[Value], tools: &Value) -> Result<Reply, ApiError> {
        self.send(messages, tools)
    }
}

impl ChatModel<'_> {
    fn send(&self, messages: &[Value], tools: &Value) -> Result<Reply, ApiError> {
        let mut body = json!({
            "model": self.model,
            "messages": cached(messages),
            "max_tokens": 1500,
            "temperature": 0,
            "verbosity": "low",
            "usage": {"include": true},
        });
        if tools.as_array().is_some_and(|tools| !tools.is_empty()) {
            body["tools"] = tools.clone();
            body["tool_choice"] = json!("auto");
            body["parallel_tool_calls"] = json!(false);
        }
        let (value, timing) = self.client.post("/chat/completions", &body)?;
        let message = value
            .pointer("/choices/0/message")
            .cloned()
            .ok_or_else(|| ApiError::Invalid("no message in the completion".into()))?;
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
        Ok(Reply {
            text,
            calls,
            message: clean(message),
            cost: value
                .pointer("/usage/cost")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            millis: timing.millis,
            model: value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            input_tokens: tokens(&value, "/usage/prompt_tokens"),
            cached_tokens: tokens(&value, "/usage/prompt_tokens_details/cached_tokens"),
            output_tokens: tokens(&value, "/usage/completion_tokens"),
        })
    }
}

/// The conversation with prompt-cache breakpoints on the system prompt and the newest message, so
/// each turn re-reads only what was added since the last one (Anthropic models through
/// OpenRouter; other providers ignore the marks or cache on their own).
fn cached(messages: &[Value]) -> Vec<Value> {
    let mark = |message: &mut Value| {
        if let Some(text) = message
            .get("content")
            .and_then(Value::as_str)
            .map(str::to_string)
        {
            message["content"] =
                json!([{"type": "text", "text": text, "cache_control": {"type": "ephemeral"}}]);
        }
    };
    let mut out = messages.to_vec();
    if let Some(system) = out.first_mut().filter(|m| m["role"] == "system") {
        mark(system);
    }
    if out.len() > 1
        && let Some(last) = out.last_mut()
    {
        mark(last);
    }
    out
}

fn tokens(value: &Value, pointer: &str) -> u64 {
    value.pointer(pointer).and_then(Value::as_u64).unwrap_or(0)
}

fn read_call(call: &Value) -> Option<ToolCall> {
    let function = call.get("function")?;
    let raw = function
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    Some(ToolCall {
        id: call.get("id")?.as_str()?.to_string(),
        name: function.get("name")?.as_str()?.to_string(),
        arguments: serde_json::from_str(if raw.trim().is_empty() { "{}" } else { raw })
            .unwrap_or(json!({})),
    })
}

/// The assistant message as the next request must carry it: role, text and tool calls only.
fn clean(message: Value) -> Value {
    let mut out = json!({"role": "assistant", "content": message.get("content").cloned().unwrap_or(Value::Null)});
    if let Some(calls) = message
        .get("tool_calls")
        .filter(|calls| calls.as_array().is_some_and(|a| !a.is_empty()))
    {
        out["tool_calls"] = calls.clone();
    }
    out
}
