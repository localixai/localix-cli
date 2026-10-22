//! The agent's loop: the chat model reads the goal, calls tools one at a time until it answers,
//! bounded by turns. A reply without a tool call is the answer. Near the end of its turns, or
//! after calls keep failing, the model is asked to answer with what it has.

use contracts::chat::{Chat, Completion, Message, Request, ToolCall, Usage};
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::Value;

use crate::answer;
use crate::conversation::Conversation;
use crate::schema;

const SYSTEM: &str = include_str!("prompt.md");
/// Failed calls in a row before the model is asked to answer.
const MAX_FAILURES: usize = 3;
/// Times a `result` that breaks its schema goes back to be fixed.
const MAX_FIXES: usize = 2;
const ANSWER_NOW: &str = "Stop calling tools and answer now from what the results showed: say \
    what is missing, and end with the RESULT line.";
const RESULT_NOW: &str = "Call `result` with the data now.";

pub struct Agent<'a> {
    pub chat: &'a dyn Chat,
    pub tools: &'a [&'a dyn Tool],
    /// Model turns for one goal.
    pub max_turns: usize,
    /// Each tool call as it is made (the live trace).
    pub on_call: &'a dyn Fn(&ToolCall),
    /// `--schema`: the run ends with data that matches it, through the `result` tool.
    pub schema: Option<&'a Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// The model answered that the goal was reached.
    Achieved,
    /// It answered that it was not.
    NotAchieved,
    /// It answered without saying.
    Unstated,
    /// Turns ran out before an answer.
    Exhausted,
    /// The model, or what a tool depends on, could not be reached.
    Error(String),
}

/// What the chat model spent in one run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spend {
    pub model: String,
    pub calls: u64,
    pub cost: f64,
    pub millis: u64,
    pub input_tokens: u64,
    pub cached_tokens: u64,
    pub output_tokens: u64,
}

impl Spend {
    fn add(&mut self, usage: &Usage) {
        self.model.clone_from(&usage.model);
        self.calls += 1;
        self.cost += usage.cost;
        self.millis += usage.millis;
        self.input_tokens += usage.input_tokens;
        self.cached_tokens += usage.cached_tokens;
        self.output_tokens += usage.output_tokens;
    }
}

pub struct Run {
    pub status: Status,
    pub answer: String,
    /// `--schema`: the data, or why none matched.
    pub data: Option<Result<Value, String>>,
    pub spend: Spend,
    /// The conversation as it ended: a follow-up goal goes on from it.
    pub conversation: Conversation,
}

impl Agent<'_> {
    /// Reach `goal` in a new conversation.
    pub fn run(&self, goal: &str) -> Run {
        self.follow_up(Conversation::new(SYSTEM), goal)
    }

    /// Reach `goal` in `conversation`, after what was done there before.
    pub fn follow_up(&self, mut conversation: Conversation, goal: &str) -> Run {
        conversation.close_open_calls();
        let brief = self
            .schema
            .map_or(String::new(), |_| format!("\n\n{}", schema::BRIEF));
        conversation.push(Message::User(format!("Goal: {goal}{brief}")));
        let mut specs: Vec<ToolSpec> = self.tools.iter().map(|tool| tool.spec()).collect();
        if let Some(schema) = self.schema {
            specs.push(schema::spec(schema));
        }
        let mut run = Run {
            status: Status::Exhausted,
            answer: String::new(),
            data: None,
            spend: Spend::default(),
            conversation,
        };
        let (mut failures, mut fixes, mut reminded) = (0, 0, false);
        for turn in 0..self.max_turns {
            let last = turn + 1 == self.max_turns;
            let wrap_up = last || failures >= MAX_FAILURES;
            if wrap_up && !reminded {
                reminded = true;
                let nudge = if self.schema.is_some() {
                    RESULT_NOW
                } else {
                    ANSWER_NOW
                };
                run.conversation.push(Message::User(nudge.into()));
            }
            let request = Request {
                conversation: run.conversation.messages(),
                tools: &specs,
                // With a schema the run ends in a call: `result`.
                may_call: !wrap_up || self.schema.is_some(),
            };
            let completion = match self.chat.complete(request) {
                Ok(completion) => completion,
                Err(error) => {
                    run.status = Status::Error(error.to_string());
                    return run;
                }
            };
            run.spend.add(&completion.usage);
            run.conversation.push(completion.message());
            if completion.calls.is_empty() {
                if self.schema.is_some() && !reminded {
                    reminded = true;
                    run.conversation.push(Message::User(RESULT_NOW.into()));
                    continue;
                }
                finish(&mut run, &completion);
                return run;
            }
            for call in &completion.calls {
                (self.on_call)(call);
                if let (schema::TOOL, Some(schema)) = (call.name.as_str(), self.schema) {
                    match schema::take(&call.arguments, schema) {
                        Ok(done) => {
                            run.status = if done.achieved {
                                Status::Achieved
                            } else {
                                Status::NotAchieved
                            };
                            run.answer = done.summary;
                            run.data = Some(Ok(done.data));
                            return run;
                        }
                        Err(problems) if fixes < MAX_FIXES => {
                            fixes += 1;
                            let why = format!(
                                "The data does not match its schema: {problems}. Call `result` \
                                 again with it fixed."
                            );
                            run.conversation.failure(&call.id, &why);
                        }
                        Err(problems) => {
                            run.status = Status::NotAchieved;
                            run.data = Some(Err(format!(
                                "the result did not match the schema: {problems}"
                            )));
                            return run;
                        }
                    }
                    continue;
                }
                match self.call(&specs, call) {
                    Ok(output) => {
                        failures = 0;
                        run.conversation.result(&call.id, output);
                    }
                    Err(ToolError::Failed(why)) => {
                        failures += 1;
                        run.conversation.failure(&call.id, &why);
                    }
                    Err(ToolError::Fatal(why)) => {
                        run.status = Status::Error(why);
                        return run;
                    }
                }
            }
        }
        run
    }

    /// Run `call` on the tool it names; `specs` are the tools' specs, in the same order.
    fn call(&self, specs: &[ToolSpec], call: &ToolCall) -> Result<ToolOutput, ToolError> {
        let tool = specs
            .iter()
            .position(|spec| spec.name == call.name)
            .and_then(|at| self.tools.get(at))
            .ok_or_else(|| ToolError::Failed(format!("there is no tool {}", call.name)))?;
        if !call.arguments.is_object() {
            return Err(ToolError::Failed(
                "the arguments are not a JSON object: call the tool again with one".into(),
            ));
        }
        tool.call(&call.arguments)
    }
}

/// The run ends with the model's answer and what its RESULT line says.
fn finish(run: &mut Run, completion: &Completion) {
    let (text, achieved) = answer::read(completion.text.as_deref().unwrap_or(""));
    run.answer = text;
    run.status = match achieved {
        Some(true) => Status::Achieved,
        Some(false) => Status::NotAchieved,
        None => Status::Unstated,
    };
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
