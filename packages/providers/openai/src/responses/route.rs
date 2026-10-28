//! Sticky routing: the backend names, in a reply's header, the machine that holds a task's
//! prompt cache. Sent back with the task's later requests, they reach that machine and read the
//! conversation so far from its cache; without it they may land anywhere and read it anew. A token
//! belongs to one task of one conversation: the next task, or another session on the same model
//! (an agent), starts without one and is given its own.

use std::sync::Mutex;

use contracts::chat::Message;
use provider_kit::http::Reply;

/// The header both ways.
pub(super) const HEADER: &str = "x-codex-turn-state";
/// How many tasks' tokens are kept: the sessions and agents at work at once, and more.
const KEPT: usize = 64;

/// The tokens of the tasks at work, the newest last.
#[derive(Default)]
pub(super) struct Routes {
    tasks: Mutex<Vec<(String, String)>>,
}

impl Routes {
    /// The token of the task `conversation` is in, when a reply gave one.
    pub(super) fn token(&self, task: &str) -> Option<String> {
        let tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
        (tasks.iter())
            .find(|(of, _)| of == task)
            .map(|(_, token)| token.clone())
    }

    /// The token `reply` gives, kept for `task` unless it has one: the first stays for the task.
    pub(super) fn keep(&self, task: String, reply: &Reply) {
        let Some(token) = reply.header(HEADER) else {
            return;
        };
        let mut tasks = self.tasks.lock().unwrap_or_else(|p| p.into_inner());
        if tasks.iter().any(|(of, _)| *of == task) {
            return;
        }
        if tasks.len() >= KEPT {
            tasks.remove(0);
        }
        tasks.push((task, token.to_string()));
    }
}

/// The task a conversation is at: its first words (the session's context and goal), how many
/// times the person spoke, and what they said last. A task begins with each of their messages.
pub(super) fn task(conversation: &[Message]) -> String {
    let said: Vec<&str> = (conversation.iter())
        .filter_map(|message| match message {
            Message::User(said) => Some(said.as_str()),
            Message::System(_)
            | Message::Assistant { .. }
            | Message::Image { .. }
            | Message::ToolResult { .. } => None,
        })
        .collect();
    let count = said.len().to_string();
    let ends = [said.first(), said.last()].into_iter().flatten().copied();
    provider_kit::cache::key(std::iter::once(count.as_str()).chain(ends))
}
