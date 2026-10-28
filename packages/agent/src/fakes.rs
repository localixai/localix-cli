//! Fakes of the ports the agent speaks to, for its tests: a scripted chat model, a scripted
//! tool and an observer that writes events down. No network, no real tool.

use std::sync::Mutex;

use contracts::chat::{Chat, ChatError, Completion, Message, Request, ToolCall, Usage};
use contracts::tool::{Input, Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use crate::conversation::Conversation;
use crate::history::{Report, Strategy};
use crate::observer::Observer;

/// A chat model that replies from a script, and remembers what each request allowed.
pub(crate) struct Scripted {
    replies: Mutex<Vec<Completion>>,
    /// Whether each request let the model call tools.
    pub(crate) may_call: Mutex<Vec<bool>>,
    /// The last user message of each request.
    pub(crate) last_user: Mutex<Vec<String>>,
}

impl Scripted {
    pub(crate) fn new(replies: Vec<Completion>) -> Self {
        Self {
            replies: Mutex::new(replies.into_iter().rev().collect()),
            may_call: Mutex::new(vec![]),
            last_user: Mutex::new(vec![]),
        }
    }
}

impl Chat for Scripted {
    #[expect(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "a test picks out the variant it checks"
    )]
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        self.may_call.lock().unwrap().push(request.may_call);
        let user = request
            .conversation
            .iter()
            .rev()
            .find_map(|message| match message {
                Message::User(text) => Some(text.clone()),
                _ => None,
            });
        self.last_user
            .lock()
            .unwrap()
            .push(user.unwrap_or_default());
        self.replies
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| ChatError::Unavailable("the script ran out".into()))
    }
}

/// An observer that writes each event down as one line.
#[derive(Default)]
pub(crate) struct Recorder(Mutex<Vec<String>>);

impl Recorder {
    pub(crate) fn events(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl Observer for Recorder {
    fn call(&self, call: &ToolCall) {
        self.0.lock().unwrap().push(format!("call {}", call.name));
    }
    fn result(&self, call: &ToolCall, outcome: &Result<ToolOutput, ToolError>) {
        let line = match outcome {
            Ok(output) => format!("ok {}: {}", call.name, output.report),
            Err(ToolError::Failed(why)) => format!("failed {}: {why}", call.name),
            Err(ToolError::Fatal(why)) => format!("fatal {}: {why}", call.name),
        };
        self.0.lock().unwrap().push(line);
    }
    fn said(&self, text: &str) {
        self.0.lock().unwrap().push(format!("said {text}"));
    }
    fn turn(&self, usage: &Usage) {
        self.0
            .lock()
            .unwrap()
            .push(format!("turn {}", usage.output_tokens));
    }
}

pub(crate) fn says(text: &str) -> Completion {
    Completion {
        text: Some(text.into()),
        ..Completion::default()
    }
}

pub(crate) fn calls(name: &str, arguments: Value) -> Completion {
    Completion {
        calls: vec![ToolCall {
            id: "c1".into(),
            name: name.into(),
            arguments,
        }],
        ..Completion::default()
    }
}

/// A tool that answers each call from a script.
pub(crate) struct Echo(Mutex<Vec<Result<ToolOutput, ToolError>>>);

impl Echo {
    pub(crate) fn new(outcomes: Vec<Result<ToolOutput, ToolError>>) -> Self {
        Self(Mutex::new(outcomes.into_iter().rev().collect()))
    }
}

impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo".into(),
            description: "Says what it did.".into(),
            input: Input::Json(json!({"type": "object"})),
        }
    }
    #[expect(
        clippy::unwrap_in_result,
        reason = "a test's fake: an unexpected call fails the test"
    )]
    fn call(&self, _: &Value) -> Result<ToolOutput, ToolError> {
        self.0.lock().unwrap().pop().expect("an unexpected call")
    }
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "a scripted tool reply, which is a call's result"
)]
pub(crate) fn ok(report: &str) -> Result<ToolOutput, ToolError> {
    Ok(ToolOutput {
        report: report.into(),
        state: Some("state".into()),
        image: None,
    })
}

/// Shows nothing.
pub(crate) struct Silent;

impl Observer for Silent {}

/// Sends the conversation as it is.
pub(crate) struct Keep;

impl Strategy for Keep {
    fn prepare(&self, _: &mut Conversation, _: u64, _: f64) -> Report {
        vec![]
    }
}
