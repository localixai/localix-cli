//! The conversation as the agent keeps it: messages that only grow, so every turn reads its whole
//! prefix from the prompt cache. Each tool result carries the state its call left; past a budget,
//! older states are dropped all at once (one cache miss, then a stable prefix again) and their
//! reports stay. A result too long for one message keeps its head and tail. A picture the person
//! attached stays.

use contracts::chat::Message;
use contracts::tool::{Image, ToolOutput};
use serde_json::Value;

use crate::history as fit;

/// Characters kept before older states are dropped (about 30k tokens).
const BUDGET: usize = 120_000;
/// Characters one tool result may take (about 22k tokens). A last resort only: each tool keeps its
/// own output under this (a command's streams), so the model is never told it saw a whole output
/// whose middle was cut here.
const MAX_RESULT: usize = 90_000;
const OMITTED: &str = "(state omitted: a newer result follows)";
/// What a picture counts for against the budget, in characters (about 1.5k tokens).
const IMAGE_SIZE: usize = 6_000;
/// What a call left without a result (a run that ended mid-turn) is answered with.
const UNANSWERED: &str = "The call was not carried out: the run ended first.";

#[derive(Debug)]
pub struct Conversation {
    messages: Vec<Message>,
    /// Each message's estimated tokens, kept with it: the estimate is asked for at every turn,
    /// and working it out means writing every call's arguments out again.
    sizes: Vec<u64>,
    /// The tool results that still carry their state: (message index, report).
    stateful: Vec<(usize, String)>,
}

impl Conversation {
    pub(crate) fn new(system: &str) -> Self {
        let system = Message::System(system.to_string());
        Self {
            sizes: vec![fit::tokens(&system)],
            messages: vec![system],
            stateful: vec![],
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// The conversation as it was when it had `len` messages: a rewind to before a task.
    pub fn truncate(&mut self, len: usize) {
        self.messages.truncate(len.max(1));
        self.sizes.truncate(self.messages.len());
        self.stateful.retain(|(at, _)| *at < self.messages.len());
    }

    pub(crate) fn push(&mut self, message: Message) {
        self.sizes.push(fit::tokens(&message));
        self.messages.push(message);
    }

    /// Message `at` changed in place: its size with it.
    fn resized(&mut self, at: usize) {
        if let (Some(size), Some(message)) = (self.sizes.get_mut(at), self.messages.get(at)) {
            *size = fit::tokens(message);
        }
    }

    /// Answer every tool call left without a result, so the conversation can go on: a model
    /// refuses one whose calls are not all answered.
    pub(crate) fn close_open_calls(&mut self) {
        let open: Vec<String> = self
            .messages
            .iter()
            .flat_map(|message| match message {
                Message::Assistant { calls, .. } => calls.iter().map(|call| call.id.clone()).collect(),
                Message::System(_)
                | Message::User(_)
                | Message::ToolResult { .. }
                | Message::Image { .. } => vec![],
            })
            .filter(|id| {
                !self.messages.iter().any(
                    |message| matches!(message, Message::ToolResult { id: answered, .. } if answered == id),
                )
            })
            .collect();
        for id in open {
            self.failure(&id, UNANSWERED);
        }
    }

    /// A tool's result for the call `id`, report then state.
    pub(crate) fn result(&mut self, id: &str, output: ToolOutput) {
        let content = shortened(&match &output.state {
            Some(state) => format!("{}\n\n{state}", output.report),
            None => output.report.clone(),
        });
        if output.state.is_some() && self.size() + content.len() > BUDGET {
            self.drop_states();
        }
        if output.state.is_some() {
            self.stateful.push((self.messages.len(), output.report));
        }
        self.push(Message::ToolResult {
            id: id.to_string(),
            content,
            failed: false,
        });
    }

    /// A picture the person attached to their message, named as they named it.
    pub(crate) fn attach(&mut self, name: &str, image: Image) {
        let caption = format!(
            "The person attached {name} ({}x{}).",
            image.width, image.height
        );
        // Kept for good: it is what the person asked about.
        self.push(Message::Image { caption, image });
    }

    /// The tokens the conversation takes, estimated.
    pub(crate) fn tokens(&self) -> u64 {
        self.sizes.iter().sum()
    }

    /// Replace message `at`'s output, or its calls' large arguments, by pointers (see `history`);
    /// how many were replaced. An elided output leaves the states too, so dropping them later
    /// never brings its report back.
    pub(crate) fn elide(&mut self, at: usize) -> usize {
        self.stateful.retain(|(state, _)| *state != at);
        let elided = match self.messages.get_mut(at) {
            Some(Message::ToolResult { content, .. }) => {
                *content = fit::pointer(content.chars().count());
                1
            }
            Some(Message::Assistant { calls, .. }) => {
                let mut elided = 0;
                for call in calls {
                    // A text tool's arguments are the text itself.
                    let values: Vec<&mut Value> = match call.arguments.is_object() {
                        true => (call.arguments.as_object_mut().into_iter())
                            .flat_map(|map| map.values_mut())
                            .collect(),
                        false => vec![&mut call.arguments],
                    };
                    for value in values {
                        if let Some(text) = value.as_str().filter(|text| fit::worth(text)) {
                            *value = Value::String(fit::argument(text.chars().count()));
                            elided += 1;
                        }
                    }
                }
                elided
            }
            _ => 0,
        };
        self.resized(at);
        elided
    }

    /// A call that did nothing useful, and why.
    pub(crate) fn failure(&mut self, id: &str, why: &str) {
        self.push(Message::ToolResult {
            id: id.to_string(),
            content: why.to_string(),
            failed: true,
        });
    }

    fn size(&self) -> usize {
        self.messages
            .iter()
            .map(|message| match message {
                Message::System(text) | Message::User(text) => text.len(),
                Message::Assistant { text, .. } => text.as_deref().map_or(0, str::len),
                Message::ToolResult { content, .. } => content.len(),
                Message::Image { caption, .. } => caption.len() + IMAGE_SIZE,
            })
            .sum()
    }

    fn drop_states(&mut self) {
        let dropped: Vec<(usize, String)> = std::mem::take(&mut self.stateful);
        for (at, report) in dropped {
            if let Some(Message::ToolResult { content, .. }) = self.messages.get_mut(at) {
                *content = format!("{report}\n{OMITTED}");
            }
            self.resized(at);
        }
    }
}

/// `text` within `MAX_RESULT` characters: its head and tail, and how much was cut between them.
fn shortened(text: &str) -> String {
    let count = text.chars().count();
    if count <= MAX_RESULT {
        return text.to_string();
    }
    let keep = MAX_RESULT / 2;
    let head: String = text.chars().take(keep).collect();
    let tail: String = text.chars().skip(count - keep).collect();
    format!("{head}\n… {} characters cut …\n{tail}", count - 2 * keep)
}

#[cfg(test)]
#[path = "conversation_tests.rs"]
mod tests;
