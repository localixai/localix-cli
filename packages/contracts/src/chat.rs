//! The chat-model port: a conversation and the tools on offer in, the model's next message out.
//! The conversation is typed here, not in any provider's wire format: an adapter maps it.

use serde_json::Value;

use crate::tool::ToolSpec;

/// One message of a conversation.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// The standing instructions.
    System(String),
    User(String),
    /// What the model said: text, tool calls, or both.
    Assistant {
        text: Option<String>,
        calls: Vec<ToolCall>,
    },
    /// What a tool call returned, for the call with `id`; `failed` when it did nothing useful.
    ToolResult {
        id: String,
        content: String,
        failed: bool,
    },
}

/// The model asking to use a tool.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// The arguments as the model wrote them: an object, or `{}` when they did not parse.
    pub arguments: Value,
}

/// The model's next message, and what it cost.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Completion {
    pub text: Option<String>,
    pub calls: Vec<ToolCall>,
    pub usage: Usage,
}

impl Completion {
    /// The completion as a message to append to the conversation.
    pub fn message(&self) -> Message {
        Message::Assistant {
            text: self.text.clone(),
            calls: self.calls.clone(),
        }
    }
}

/// What one completion took.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Usage {
    /// The model that answered, as the provider names it.
    pub model: String,
    pub cost: f64,
    pub millis: u64,
    pub input_tokens: u64,
    /// Input tokens read from the provider's prompt cache.
    pub cached_tokens: u64,
    pub output_tokens: u64,
}

/// Why no completion came back.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatError {
    /// The model could not be reached or refused the call (network, key, quota).
    Unavailable(String),
    /// It answered, but not with something usable.
    Invalid(String),
}

impl std::fmt::Display for ChatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message) | Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ChatError {}

/// One request: the conversation so far and the tools it may call.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    pub conversation: &'a [Message],
    /// Offered on every request, so the conversation's earlier calls stay valid.
    pub tools: &'a [ToolSpec],
    /// Whether the model may call them now; `false` asks for an answer.
    pub may_call: bool,
}

pub trait Chat {
    fn complete(&self, request: Request) -> Result<Completion, ChatError>;
}
