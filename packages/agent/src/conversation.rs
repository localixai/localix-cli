//! The conversation as the agent keeps it: messages that only grow, so every turn reads its whole
//! prefix from the prompt cache. Each tool result carries the state its call left; past a budget,
//! older states are dropped all at once (one cache miss, then a stable prefix again) and their
//! reports stay. A result too long for one message keeps its head and tail.

use contracts::chat::Message;
use contracts::tool::ToolOutput;

/// Characters kept before older states are dropped (about 30k tokens).
const BUDGET: usize = 120_000;
/// Characters one tool result may take (about 6k tokens).
const MAX_RESULT: usize = 24_000;
const OMITTED: &str = "(state omitted: a newer result follows)";
/// What a call left without a result (a run that ended mid-turn) is answered with.
const UNANSWERED: &str = "The call was not carried out: the run ended first.";

#[derive(Debug, Clone, Default)]
pub struct Conversation {
    messages: Vec<Message>,
    /// The tool results that still carry their state: (message index, report).
    stateful: Vec<(usize, String)>,
}

impl Conversation {
    pub fn new(system: &str) -> Self {
        Self {
            messages: vec![Message::System(system.to_string())],
            stateful: vec![],
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Answer every tool call left without a result, so the conversation can go on: a model
    /// refuses one whose calls are not all answered.
    pub fn close_open_calls(&mut self) {
        let open: Vec<String> = self
            .messages
            .iter()
            .flat_map(|message| match message {
                Message::Assistant { calls, .. } => calls.iter().map(|call| call.id.clone()).collect(),
                _ => vec![],
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
    pub fn result(&mut self, id: &str, output: ToolOutput) {
        let content = shortened(&match &output.state {
            Some(state) => format!("{}\n\n{state}", output.report),
            None => output.report.clone(),
        });
        if self.size() + content.len() > BUDGET {
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

    /// A call that did nothing useful, and why.
    pub fn failure(&mut self, id: &str, why: &str) {
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
            })
            .sum()
    }

    fn drop_states(&mut self) {
        for (at, report) in self.stateful.drain(..) {
            if let Some(Message::ToolResult { content, .. }) = self.messages.get_mut(at) {
                *content = format!("{report}\n{OMITTED}");
            }
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
