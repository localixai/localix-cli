//! The chat port over OpenAI's chat completions, with tool calling: OpenRouter's models, and any
//! provider's that speaks the same wire. OpenRouter takes its own `reasoning` object, a usage
//! report with the cost, and prompt-cache marks for the models that take them; the others take `reasoning_effort`, and OpenAI's
//! own API the prompt cache's key.

use contracts::catalog::{Effort, Selection};
use contracts::chat::{Calling, Chat, ChatError, Completion, Message, Request, ToolCall, Usage};
use contracts::tool::{Image, ToolSpec};
use serde_json::{Value, json};

use crate::client::{ApiError, OpenRouter, Wire};

pub struct ChatModel {
    pub client: OpenRouter,
    pub selection: Selection,
}

impl Chat for ChatModel {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        let model = &self.selection.model().id;
        let body = body(self.client.wire(), &self.selection, &request);
        let (value, millis) = self
            .client
            .post("/chat/completions", &body, request.deadline)
            .map_err(|error| match error {
                ApiError::Invalid(message) => ChatError::Invalid(message),
                other @ (ApiError::Auth(_) | ApiError::Http(..) | ApiError::Network(_)) => {
                    ChatError::Unavailable(format!("{} {other}", self.client.name))
                }
            })?;
        let mut completion = read(&value, model, millis)?;
        completion.calls = (completion.calls.into_iter())
            .map(|call| incoming(call, request.tools))
            .collect();
        Ok(completion)
    }
}

/// A call the model made, as the agent takes it: a patch's text taken out of its one argument.
fn incoming(mut call: ToolCall, tools: &[ToolSpec]) -> ToolCall {
    if let Some(spec) = tools.iter().find(|spec| spec.name == call.name) {
        call.arguments = spec.input.from_function(call.arguments);
    }
    call
}

/// The completion in a reply: its text, its calls, the reasoning that goes back with it, and the
/// usage.
fn read(value: &Value, model: &str, millis: u64) -> Result<Completion, ChatError> {
    let message = value
        .pointer("/choices/0/message")
        .ok_or_else(|| ChatError::Invalid("no message in the completion".into()))?;
    // Who served it (OpenRouter routes a model to one of its hosts) and the calls as they came:
    // what a call that reached a tool malformed is traced back to.
    log::debug!(
        "{} served {}: {}",
        value["provider"].as_str().unwrap_or("the host"),
        value["model"].as_str().unwrap_or(model),
        message.get("tool_calls").unwrap_or(&Value::Null)
    );
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
        carried: thought(message, model).into_iter().collect(),
        usage: Usage {
            model: value["model"].as_str().unwrap_or(model).to_string(),
            cost: value
                .pointer("/usage/cost")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            millis,
            input_tokens: tokens("/usage/prompt_tokens"),
            cached_tokens: tokens("/usage/prompt_tokens_details/cached_tokens"),
            output_tokens: tokens("/usage/completion_tokens"),
        },
    })
}

/// The fields a host says its model's reasoning in, the first that holds some taken: a host may
/// say it twice, in two of them.
const THOUGHT: [&str; 3] = ["reasoning_content", "reasoning", "reasoning_text"];

/// The reasoning a message holds, as it goes back: OpenRouter's `reasoning_details` whole (signed
/// or sealed parts a model checks), else the text with the field it came in; tagged with the model
/// that wrote it, which alone may read it.
fn thought(message: &Value, model: &str) -> Option<Value> {
    if let Some(details) = message
        .get("reasoning_details")
        .filter(|details| details.as_array().is_some_and(|parts| !parts.is_empty()))
    {
        return Some(json!({"model": model, "fields": {"reasoning_details": details}}));
    }
    THOUGHT.iter().find_map(|field| {
        let text = message
            .get(*field)?
            .as_str()
            .filter(|text| !text.is_empty())?;
        Some(json!({"model": model, "fields": {*field: text}}))
    })
}

/// The request's body: OpenRouter's own fields, or the generic ones.
fn body(host: Wire, selection: &Selection, request: &Request) -> Value {
    let model = &selection.model().id;
    let messages: Vec<Value> = (request.conversation.iter())
        .map(|message| wire(message, model, request.tools))
        .collect();
    // No output limit: a whole file in one call is a long reply, and a cut one breaks the
    // call's arguments. The model's own maximum applies.
    let mut body = json!({"model": model});
    // Only a model that takes levels is sent one: another may refuse the field.
    let levels = !selection.model().efforts.is_empty();
    if host == Wire::OpenRouter {
        body["messages"] = json!(if selection.model().marks {
            cached(messages)
        } else {
            messages
        });
        if levels {
            body["reasoning"] = reasoning(selection.effort());
        }
        body["usage"] = json!({"include": true});
    } else {
        body["messages"] = json!(messages);
        if levels {
            body["reasoning_effort"] = json!(selection.effort().wire_name());
        }
    }
    if host == Wire::OpenAi {
        body["prompt_cache_key"] = json!(cache_key(model, request));
    }
    if !request.tools.is_empty() {
        body["tools"] = request.tools.iter().map(function).collect();
        body["tool_choice"] = match request.calling {
            Calling::Any => json!("auto"),
            Calling::Nothing => json!("none"),
            Calling::Only(name) => json!({"type": "function", "function": {"name": name}}),
        };
        // Several calls in one turn: independent reads and searches cost one turn, not many.
        // The agent runs together only the calls whose tools allow it, the rest in order.
        body["parallel_tool_calls"] = json!(true);
    }
    body
}

/// The prompt cache's key: the model, the tools and the system prompt, the start every request of
/// every session with them shares.
fn cache_key(model: &str, request: &Request) -> String {
    let tools = request.tools.iter().flat_map(|spec| {
        [
            spec.name.clone(),
            spec.description.clone(),
            spec.input.schema().to_string(),
        ]
    });
    let rules = request
        .conversation
        .iter()
        .map_while(|message| match message {
            Message::System(rules) => Some(rules.clone()),
            Message::User(_)
            | Message::Assistant { .. }
            | Message::Image { .. }
            | Message::ToolResult { .. } => None,
        });
    provider_kit::cache::key(std::iter::once(model.to_string()).chain(tools).chain(rules))
}

/// The level as OpenRouter's `reasoning` takes it. Off is no thinking at all: it costs seconds
/// per turn, and the agent reasons in its tool calls instead.
pub(crate) fn reasoning(effort: Effort) -> Value {
    match effort {
        Effort::Off => json!({"enabled": false}),
        effort @ (Effort::Minimal
        | Effort::Low
        | Effort::Medium
        | Effort::High
        | Effort::XHigh
        | Effort::Max
        | Effort::Ultra) => json!({"effort": effort.name()}),
    }
}

/// A message in the chat-completions wire format, for `model`: the reasoning another model wrote
/// stays out.
fn wire(message: &Message, model: &str, tools: &[ToolSpec]) -> Value {
    match message {
        Message::System(text) => json!({"role": "system", "content": text}),
        Message::User(text) => json!({"role": "user", "content": text}),
        Message::Assistant {
            text,
            calls,
            carried,
        } => {
            let mut out = json!({"role": "assistant", "content": text});
            // Only what this provider kept: another's (a Responses reasoning) has no "fields".
            let kept = carried.iter().filter(|kept| kept["model"] == model);
            for (field, value) in kept.filter_map(|kept| kept["fields"].as_object()).flatten() {
                out[field.as_str()] = value.clone();
            }
            if !calls.is_empty() {
                out["tool_calls"] = calls
                    .iter()
                    .map(|call| {
                        // A patch's text goes back in the one argument it was offered with.
                        let arguments = match tools.iter().find(|spec| spec.name == call.name) {
                            Some(spec) => spec.input.to_function(&call.arguments),
                            None => call.arguments.clone(),
                        };
                        json!({"id": call.id, "type": "function", "function": {
                            "name": call.name, "arguments": arguments.to_string()}})
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
        "name": spec.name, "description": spec.description, "parameters": spec.input.schema()}})
}

/// The conversation with prompt-cache breakpoints on the system prompt and the newest message, so
/// each turn re-reads only what was added since the last one.
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
