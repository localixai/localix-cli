//! The agent's loop: the chat model reads the goal, calls tools until it answers, bounded by
//! turns. A reply without a tool call is the answer. Near the end of its turns, or after calls keep
//! failing, the model is asked to answer with what it has.

use contracts::chat::{Chat, Completion, Message, Request, ToolCall, Usage};
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::Value;

use crate::answer;
use crate::conversation::Conversation;
use crate::observer::Observer;
use crate::schema;

const SYSTEM: &str = include_str!("prompt.md");
/// Failed calls in a row before the model is asked to answer.
const MAX_FAILURES: usize = 3;
/// Times a `result` that breaks its schema goes back to be fixed.
const MAX_FIXES: usize = 2;
const ANSWER_NOW: &str = "Stop calling tools and answer now from what the results showed: say \
    what is missing, and end with the RESULT line.";
const RESULT_NOW: &str = "Call `result` with the data now.";
/// The result a matching `result` call gets, so a follow-up finds no call left open.
const RETURNED: &str = "The result was returned.";
/// Why a run with a schema has no data, when the model kept answering in text.
const NO_DATA: &str = "the model answered in text, without the data its schema asks for";

pub struct Agent<'a> {
    pub chat: &'a dyn Chat,
    pub tools: &'a [&'a dyn Tool],
    /// What the app knows of where the run happens (the working directory, the system, the date,
    /// the project's instructions): it opens a new conversation, before the first goal. Empty for
    /// none.
    pub context: &'a str,
    /// Model turns for one goal.
    pub max_turns: usize,
    /// Sees each call, its result and each turn as they happen (the live trace).
    pub observer: &'a dyn Observer,
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

/// What one goal has used of its second chances.
#[derive(Default)]
struct Tally {
    /// Failed calls in a row.
    failures: usize,
    /// `result` calls sent back to be fixed.
    fixes: usize,
    /// Whether the model was told to answer now: it is told once.
    reminded: bool,
}

impl Agent<'_> {
    /// Reach `goal` in a new conversation, which opens with the context.
    pub fn run(&self, goal: &str) -> Run {
        let context = self.context.trim();
        let opening = if context.is_empty() {
            self.goal(goal)
        } else {
            format!("<context>\n{context}\n</context>\n\n{}", self.goal(goal))
        };
        self.reach(Conversation::new(SYSTEM), opening)
    }

    /// Reach `goal` in `conversation`, after what was done there before. The context is not
    /// repeated: the conversation already opens with it.
    pub fn follow_up(&self, conversation: Conversation, goal: &str) -> Run {
        self.reach(conversation, self.goal(goal))
    }

    /// The goal as the model reads it, with what a schema asks of the end.
    fn goal(&self, goal: &str) -> String {
        match self.schema {
            Some(_) => format!("Goal: {goal}\n\n{}", schema::BRIEF),
            None => format!("Goal: {goal}"),
        }
    }

    fn reach(&self, mut conversation: Conversation, message: String) -> Run {
        conversation.close_open_calls();
        conversation.push(Message::User(message));
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
        let mut tally = Tally::default();
        for turn in 0..self.max_turns {
            let wrap_up = turn + 1 == self.max_turns || tally.failures >= MAX_FAILURES;
            if wrap_up {
                self.remind(&mut run, &mut tally);
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
            self.observer.turn(&completion.usage);
            run.spend.add(&completion.usage);
            run.conversation.push(completion.message());
            if completion.calls.is_empty() {
                // With a schema, text alone is not the end: the model is asked once for `result`.
                if self.schema.is_some() && !tally.reminded {
                    self.remind(&mut run, &mut tally);
                    continue;
                }
                finish(&mut run, &completion);
                if self.schema.is_some() {
                    // Asked for `result` and answered in text again: there is no data, and the
                    // run says so rather than passing prose off as the answer a pipeline expects.
                    run.status = Status::NotAchieved;
                    run.data = Some(Err(NO_DATA.into()));
                }
                return run;
            }
            for call in &completion.calls {
                if self.handle(call, &specs, &mut run, &mut tally) {
                    return run;
                }
            }
        }
        run
    }

    /// Tell the model, once, to end: with its answer, or with `result`.
    fn remind(&self, run: &mut Run, tally: &mut Tally) {
        if tally.reminded {
            return;
        }
        tally.reminded = true;
        let nudge = match self.schema {
            Some(_) => RESULT_NOW,
            None => ANSWER_NOW,
        };
        run.conversation.push(Message::User(nudge.into()));
    }

    /// Carry out `call`, answer it in the conversation, and say whether the run ends with it.
    fn handle(
        &self,
        call: &ToolCall,
        specs: &[ToolSpec],
        run: &mut Run,
        tally: &mut Tally,
    ) -> bool {
        self.observer.call(call);
        let (outcome, ends) = match (call.name.as_str(), self.schema) {
            (schema::TOOL, Some(schema)) => take_result(call, schema, run, &mut tally.fixes),
            _ => {
                let outcome = self.call(specs, call);
                match &outcome {
                    Ok(_) => tally.failures = 0,
                    Err(ToolError::Failed(_)) => tally.failures += 1,
                    Err(ToolError::Fatal(why)) => run.status = Status::Error(why.clone()),
                }
                let ends = matches!(outcome, Err(ToolError::Fatal(_)));
                (outcome, ends)
            }
        };
        self.observer.result(call, &outcome);
        match outcome {
            Ok(output) => run.conversation.result(&call.id, output),
            Err(error) => run.conversation.failure(&call.id, &error.to_string()),
        }
        ends
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

/// A `result` call: data that matches ends the run; a mismatch goes back to be fixed, until the
/// fixes run out and the run ends without data. Returns its outcome and whether the run ends.
fn take_result(
    call: &ToolCall,
    schema: &Value,
    run: &mut Run,
    fixes: &mut usize,
) -> (Result<ToolOutput, ToolError>, bool) {
    match schema::take(&call.arguments, schema) {
        Ok(done) => {
            run.status = if done.achieved {
                Status::Achieved
            } else {
                Status::NotAchieved
            };
            run.answer = done.summary;
            run.data = Some(Ok(done.data));
            let output = ToolOutput {
                report: RETURNED.into(),
                state: None,
            };
            (Ok(output), true)
        }
        Err(problems) if *fixes < MAX_FIXES => {
            *fixes += 1;
            let why = format!(
                "The data does not match its schema: {problems}. Call `result` again with it fixed."
            );
            (Err(ToolError::Failed(why)), false)
        }
        Err(problems) => {
            let why = format!("the result did not match the schema: {problems}");
            run.status = Status::NotAchieved;
            run.data = Some(Err(why.clone()));
            (Err(ToolError::Failed(why)), true)
        }
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
