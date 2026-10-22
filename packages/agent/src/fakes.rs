//! Fakes of the ports the agent speaks to, for its tests: a scripted chat model, a scripted
//! tool and an observer that writes events down. No network, no real tool.

use std::cell::RefCell;

use contracts::chat::{Chat, ChatError, Completion, Message, Request, ToolCall, Usage};
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use crate::observer::Observer;

/// A chat model that replies from a script, and remembers what each request allowed.
pub struct Scripted {
    replies: RefCell<Vec<Completion>>,
    /// Whether each request let the model call tools.
    pub may_call: RefCell<Vec<bool>>,
    /// The last user message of each request.
    pub last_user: RefCell<Vec<String>>,
}

impl Scripted {
    pub fn new(replies: Vec<Completion>) -> Self {
        Self {
            replies: RefCell::new(replies.into_iter().rev().collect()),
            may_call: RefCell::new(vec![]),
            last_user: RefCell::new(vec![]),
        }
    }
}

impl Chat for Scripted {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        self.may_call.borrow_mut().push(request.may_call);
        let user = request
            .conversation
            .iter()
            .rev()
            .find_map(|message| match message {
                Message::User(text) => Some(text.clone()),
                _ => None,
            });
        self.last_user.borrow_mut().push(user.unwrap_or_default());
        self.replies
            .borrow_mut()
            .pop()
            .ok_or_else(|| ChatError::Unavailable("the script ran out".into()))
    }
}

/// An observer that writes each event down as one line.
#[derive(Default)]
pub struct Recorder(RefCell<Vec<String>>);

impl Recorder {
    pub fn events(&self) -> Vec<String> {
        self.0.borrow().clone()
    }
}

impl Observer for Recorder {
    fn call(&self, call: &ToolCall) {
        self.0.borrow_mut().push(format!("call {}", call.name));
    }
    fn result(&self, call: &ToolCall, outcome: &Result<ToolOutput, ToolError>) {
        let line = match outcome {
            Ok(output) => format!("ok {}: {}", call.name, output.report),
            Err(ToolError::Failed(why)) => format!("failed {}: {why}", call.name),
            Err(ToolError::Fatal(why)) => format!("fatal {}: {why}", call.name),
        };
        self.0.borrow_mut().push(line);
    }
    fn turn(&self, usage: &Usage) {
        self.0
            .borrow_mut()
            .push(format!("turn {}", usage.output_tokens));
    }
}

pub fn says(text: &str) -> Completion {
    Completion {
        text: Some(text.into()),
        ..Completion::default()
    }
}

pub fn calls(name: &str, arguments: Value) -> Completion {
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
pub struct Echo(RefCell<Vec<Result<ToolOutput, ToolError>>>);

impl Echo {
    pub fn new(outcomes: Vec<Result<ToolOutput, ToolError>>) -> Self {
        Self(RefCell::new(outcomes.into_iter().rev().collect()))
    }
}

impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo".into(),
            description: "Says what it did.".into(),
            parameters: json!({"type": "object"}),
        }
    }
    fn call(&self, _: &Value) -> Result<ToolOutput, ToolError> {
        self.0.borrow_mut().pop().expect("an unexpected call")
    }
}

pub fn ok(report: &str) -> Result<ToolOutput, ToolError> {
    Ok(ToolOutput {
        report: report.into(),
        state: Some("state".into()),
    })
}
