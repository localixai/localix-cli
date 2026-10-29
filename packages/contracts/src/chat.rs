//! The chat-model port: a conversation and the tools on offer in, the model's next message out.
//! The conversation is typed here, not in any provider's wire format: an adapter maps it.

use serde_json::Value;

use crate::tool::{Image, ToolSpec};

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
        /// What the provider asked to have sent back with this turn, as it gave it, opaque to
        /// everyone else: a model's reasoning, sealed, that it would otherwise lose between turns.
        carried: Vec<Value>,
    },
    /// What a tool call returned, for the call with `id`; `failed` when it did nothing useful.
    ToolResult {
        id: String,
        content: String,
        failed: bool,
    },
    /// A picture the person attached to their message. `caption` says what it shows.
    Image {
        caption: String,
        image: Image,
    },
}

/// The model asking to use a tool.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// The arguments as the model wrote them: an object (the text itself for a text tool), or
    /// the text itself when they did not parse.
    pub arguments: Value,
}

/// The model's next message, and what it cost.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Completion {
    pub text: Option<String>,
    pub calls: Vec<ToolCall>,
    /// What goes back with this turn (`Message::Assistant::carried`).
    pub carried: Vec<Value>,
    pub usage: Usage,
}

impl Completion {
    /// The completion as a message to append to the conversation.
    pub fn message(&self) -> Message {
        Message::Assistant {
            text: self.text.clone(),
            calls: self.calls.clone(),
            carried: self.carried.clone(),
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

/// What a model spent over a run: its calls' usage, summed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spend {
    /// The model that answered last, as its provider names it.
    pub model: String,
    pub calls: u64,
    pub cost: f64,
    pub millis: u64,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub output_tokens: u64,
}

impl Spend {
    /// One more call's usage. An answer that names no model keeps the one before.
    pub fn add(&mut self, usage: &Usage) {
        if !usage.model.is_empty() {
            self.model.clone_from(&usage.model);
        }
        self.calls += 1;
        self.cost += usage.cost;
        self.millis += usage.millis;
        self.input_tokens += usage.input_tokens;
        self.cached_tokens += usage.cached_tokens;
        self.output_tokens += usage.output_tokens;
    }

    /// Add what another run spent (an agent this one started): its turns and tokens count here.
    pub fn absorb(&mut self, other: &Spend) {
        if self.model.is_empty() {
            self.model.clone_from(&other.model);
        }
        self.calls += other.calls;
        self.cost += other.cost;
        self.millis += other.millis;
        self.input_tokens += other.input_tokens;
        self.cached_tokens += other.cached_tokens;
        self.output_tokens += other.output_tokens;
    }
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

pub trait Chat: Send + Sync {
    fn complete(&self, request: Request) -> Result<Completion, ChatError>;
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;
