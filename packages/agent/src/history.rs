//! How a conversation is prepared for the model before each request: a [`Strategy`], chosen by
//! the app. Each participant (the main agent, each agent it
//! started, a future group's members) keeps its own conversation and is fitted to its own
//! model, so one strategy serves them all; a shared history would be laid out into each
//! participant's conversation first, then fitted.
//!
//! [`Pipeline`] is the strategy llx uses: named [`Stage`]s, least valuable first, each eliding
//! what it owns (outputs, arguments) for pointers that say how to get it back. No summary. Made
//! for the prompt cache:
//! - nothing runs while the conversation is under `high` of the model's limit (a pruned prefix
//!   trades cheap cached tokens for a rewrite of everything after it);
//! - once it is, the stages run until `low`, so one cache miss buys many stable turns;
//! - an elision is made in the conversation itself, so it is never undone (a ratchet);
//! - the recent tail, `tail` tokens and at least `MIN_TAIL` messages, is left alone unless the
//!   limit itself is passed.

use std::collections::HashSet;

use contracts::chat::Message;
use serde_json::Value;

use crate::conversation::Conversation;

/// The window assumed when the model's is not known.
const FALLBACK: u64 = 128_000;
/// The most a request holds, whatever the window: quality and cost fall well before a million.
const AUTO: u64 = 272_000;
const MIN_TAIL: usize = 4;
/// Characters below which nothing is worth eliding.
const MIN_CHARS: usize = 400;
pub const MARKER: &str = "elided to save context";
/// What a picture counts for, in tokens.
const IMAGE: u64 = 1_500;

/// The arithmetic of one request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// The most the request may hold, in tokens.
    pub ceiling: u64,
    /// Where a pass stops.
    pub target: u64,
    /// Real tokens per estimated one, from the last answer's usage.
    pub ratio: f64,
    /// Where the protected tail starts.
    pub tail: usize,
}

impl Frame {
    /// The conversation's size in real tokens, as far as the estimate and the ratio tell.
    pub fn size(&self, conversation: &Conversation) -> u64 {
        whole(conversation.tokens() as f64 * self.ratio)
    }
}

/// What a preparation did: each stage that changed something, and how much.
pub type Report = Vec<(&'static str, usize)>;

pub trait Strategy: Send + Sync {
    /// The most a request to a model with a `window` of tokens may hold.
    fn ceiling(&self, window: u64) -> u64 {
        let window = if window == 0 { FALLBACK } else { window };
        // A reserve for the answer and the growth within a run.
        let reserve = (window / 5).min(16_384).max(window / 10);
        (window - reserve).min(AUTO)
    }

    /// Prepare `conversation` for a request that may hold `ceiling` tokens.
    fn prepare(&self, conversation: &mut Conversation, ceiling: u64, ratio: f64) -> Report;
}

/// One step of a pipeline: elides what it owns before `before`, oldest first, until the
/// conversation is down to the frame's target; how much it elided.
pub trait Stage: Send + Sync {
    fn name(&self) -> &'static str;
    fn run(&self, conversation: &mut Conversation, frame: &Frame) -> usize;
}

/// Sends the conversation as it is.
pub struct Keep;

impl Strategy for Keep {
    fn prepare(&self, _: &mut Conversation, _: u64, _: f64) -> Report {
        vec![]
    }
}

/// Stages behind a gate, in order.
pub struct Pipeline {
    pub stages: Vec<Box<dyn Stage>>,
    /// Percent of the ceiling that opens the gate, and where a pass stops.
    pub high: u64,
    pub low: u64,
    /// Tokens of the recent tail left alone.
    pub tail: u64,
}

impl Default for Pipeline {
    /// llx's: superseded outputs, old outputs, old arguments; the tail only past the ceiling.
    fn default() -> Self {
        Self {
            stages: vec![
                Box::new(Outputs { superseded: true }),
                Box::new(Outputs { superseded: false }),
                Box::new(Arguments),
            ],
            high: 90,
            low: 60,
            tail: 16_384,
        }
    }
}

impl Strategy for Pipeline {
    fn prepare(&self, conversation: &mut Conversation, ceiling: u64, ratio: f64) -> Report {
        let mut frame = Frame {
            ceiling,
            target: ceiling * self.low / 100,
            ratio,
            // The tail is counted in real tokens too.
            tail: tail(conversation, whole(self.tail as f64 / ratio.max(0.01))),
        };
        let mut report = Report::new();
        if frame.size(conversation) * 100 < ceiling * self.high {
            return report;
        }
        let mut run = |frame: &Frame, conversation: &mut Conversation| {
            for stage in &self.stages {
                if frame.size(conversation) <= frame.target {
                    break;
                }
                let elided = stage.run(conversation, frame);
                if elided > 0 {
                    report.push((stage.name(), elided));
                }
            }
        };
        run(&frame, conversation);
        // Still past the ceiling itself: the tail gives way too, all but its last messages.
        if frame.size(conversation) > ceiling {
            frame.tail = conversation.messages().len().saturating_sub(MIN_TAIL);
            run(&frame, conversation);
        }
        report
    }
}

/// Tool outputs: those a later call on the same `path` superseded, or any.
pub struct Outputs {
    pub superseded: bool,
}

impl Stage for Outputs {
    fn name(&self) -> &'static str {
        if self.superseded {
            "superseded"
        } else {
            "outputs"
        }
    }

    fn run(&self, conversation: &mut Conversation, frame: &Frame) -> usize {
        let later = superseded(conversation.messages());
        let owned: Vec<usize> = conversation.messages()[..frame.tail]
            .iter()
            .enumerate()
            .filter(|(_, message)| match message {
                Message::ToolResult { id, content, .. } => {
                    worth(content) && (!self.superseded || later.contains(id))
                }
                _ => false,
            })
            .map(|(at, _)| at)
            .collect();
        elide(conversation, frame, owned)
    }
}

/// The large arguments of tool calls: what an edit wrote is on disk.
pub struct Arguments;

impl Stage for Arguments {
    fn name(&self) -> &'static str {
        "arguments"
    }

    fn run(&self, conversation: &mut Conversation, frame: &Frame) -> usize {
        let owned: Vec<usize> = conversation.messages()[..frame.tail]
            .iter()
            .enumerate()
            .filter(|(_, message)| {
                matches!(message, Message::Assistant { calls, .. }
                    if calls.iter().any(|call| large(&call.arguments)))
            })
            .map(|(at, _)| at)
            .collect();
        elide(conversation, frame, owned)
    }
}

/// Elide the messages at `owned`, oldest first, until the frame's target.
fn elide(conversation: &mut Conversation, frame: &Frame, owned: Vec<usize>) -> usize {
    let mut elided = 0;
    for at in owned {
        if frame.size(conversation) <= frame.target {
            break;
        }
        elided += conversation.elide(at);
    }
    elided
}

/// Where the tail of `budget` tokens starts: at least `MIN_TAIL` messages from the end.
fn tail(conversation: &Conversation, budget: u64) -> usize {
    let messages = conversation.messages();
    let (mut size, mut start) = (0, messages.len());
    while start > 0 {
        size += tokens(&messages[start - 1]);
        if size > budget && messages.len() - start >= MIN_TAIL {
            break;
        }
        start -= 1;
    }
    start
}

/// The calls whose output a later call on the same `path` superseded.
fn superseded(messages: &[Message]) -> HashSet<String> {
    let (mut later, mut out) = (HashSet::new(), HashSet::new());
    for message in messages.iter().rev() {
        if let Message::Assistant { calls, .. } = message {
            for call in calls {
                let path = call.arguments.get("path").and_then(Value::as_str);
                if path.is_some_and(|path| !later.insert(path.to_string())) {
                    out.insert(call.id.clone());
                }
            }
        }
    }
    out
}

pub(crate) fn worth(text: &str) -> bool {
    text.len() >= MIN_CHARS && !text.contains(MARKER)
}

fn large(arguments: &Value) -> bool {
    arguments
        .as_object()
        .is_some_and(|map| map.values().any(|v| v.as_str().is_some_and(worth)))
}

/// An estimate in whole tokens.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "`as` saturates and drops the fraction: whole tokens are meant, and none below zero"
)]
pub(crate) fn whole(tokens: f64) -> u64 {
    tokens as u64
}

/// A message's tokens, estimated: four characters to a token.
pub(crate) fn tokens(message: &Message) -> u64 {
    let chars = match message {
        Message::System(text) | Message::User(text) => text.len(),
        Message::Assistant { text, calls } => {
            let arguments: usize = calls.iter().map(|c| c.arguments.to_string().len()).sum();
            text.as_deref().map_or(0, str::len) + arguments
        }
        Message::ToolResult { content, .. } => content.len(),
        Message::Image { .. } => return IMAGE,
    };
    chars as u64 / 4 + 1
}

/// The pointer left in place of an output of `chars` characters.
pub fn pointer(chars: usize) -> String {
    format!(
        "[output {MARKER}: {chars} characters. Call the tool again if you still need it; if the \
         file changed since, read it again rather than relying on what was here]"
    )
}

/// The pointer left in place of an argument of `chars` characters.
pub fn argument(chars: usize) -> String {
    format!("[{MARKER}: {chars} characters]")
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
