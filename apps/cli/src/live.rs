//! The models a session thinks with, switchable between tasks: `/login` puts in the chat model
//! it chose (and the browser's decision model, once there is a key for one) without rebuilding
//! the agent, its tools or its conversation.

use std::collections::BTreeMap;
use std::sync::RwLock;

use contracts::chat::{Chat, ChatError, Completion, Request};
use contracts::model::{DecideError, Decider, Decision, Question};
use serde_json::Value;

use crate::choice::Choice;
use crate::providers::{self, Provider};

pub struct Live {
    pub chat: LiveChat,
    pub decider: LiveDecider,
}

impl Live {
    /// The models of `choice` (one that asks to sign in, when there is no sign-in yet) and the
    /// decision model `decision`, where there is a key for it.
    pub fn of(choice: &Choice, decision: &str) -> Self {
        let chat = choice
            .provider
            .chat(&choice.selection)
            .unwrap_or_else(|| Box::new(SignedOut(choice.provider.clone())));
        Self {
            chat: LiveChat::new(chat),
            decider: LiveDecider::new(providers::decider(decision)),
        }
    }
}

pub struct LiveChat(RwLock<Box<dyn Chat>>);

impl LiveChat {
    pub fn new(chat: Box<dyn Chat>) -> Self {
        Self(RwLock::new(chat))
    }

    /// Think with `chat` from the next request on.
    pub fn set(&self, chat: Box<dyn Chat>) {
        *self.0.write().unwrap_or_else(|poison| poison.into_inner()) = chat;
    }
}

impl Chat for LiveChat {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        let chat = self.0.read().unwrap_or_else(|poison| poison.into_inner());
        chat.complete(request)
    }
}

pub struct LiveDecider(RwLock<Option<Box<dyn Decider>>>);

/// Why the browser cannot act without an OpenRouter key.
pub const NO_DECIDER: &str = "the browser's decision model runs on OpenRouter: set \
OPENROUTER_API_KEY, or type /login and add an OpenRouter key";

impl LiveDecider {
    pub fn new(decider: Option<Box<dyn Decider>>) -> Self {
        Self(RwLock::new(decider))
    }

    pub fn set(&self, decider: Option<Box<dyn Decider>>) {
        *self.0.write().unwrap_or_else(|poison| poison.into_inner()) = decider;
    }
}

impl Decider for LiveDecider {
    fn decide(
        &self,
        state: &Value,
        questions: &BTreeMap<String, Question>,
    ) -> Result<Decision, DecideError> {
        let decider = self.0.read().unwrap_or_else(|poison| poison.into_inner());
        match decider.as_ref() {
            Some(decider) => decider.decide(state, questions),
            None => Err(DecideError::Unavailable(NO_DECIDER.into())),
        }
    }
}

/// The chat model of a session not signed in yet: every request says how to sign in.
pub struct SignedOut(pub Provider);

impl Chat for SignedOut {
    fn complete(&self, _: Request) -> Result<Completion, ChatError> {
        Err(ChatError::Unavailable(format!(
            "not signed in to {}: type /login",
            self.0.name()
        )))
    }
}

#[cfg(test)]
#[path = "live_tests.rs"]
mod tests;
