//! The agent's loop: the chat model reads the goal and calls tools until it answers with a
//! message and no call, bounded by `max_turns` (unlimited unless the app sets one). That message
//! is the answer; with `--schema`, the data is then asked for in one more request with the
//! `result` call forced ([`crate::schema`]). Nothing pushes the model on or invites it to stop: a
//! failed call goes back to it with its reason, and no tool ends the task.
//! The calls of one turn that may run together do ([`crate::calls`]). Before each turn, what
//! came into the inbox is said; an answer while something is still to come waits for it, and the
//! run goes on.

mod handling;

use contracts::chat::{Calling, Chat, ChatError, Completion, Message, Request, Spend};
use contracts::inbox::Inbox;
use contracts::tool::{Image, Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::Value;

use crate::conversation::Conversation;
use crate::history::{Strategy, whole};
use crate::observer::Observer;
use crate::schema;

const SYSTEM: &str = include_str!("prompt.md");
/// Times a `result` that breaks its schema goes back to be fixed.
const MAX_FIXES: usize = 2;
/// Asked once the work has ended, with `--schema`: the request that forces the `result` call.
const DATA_NOW: &str = "Return the data the schema asks for, as `result`.";
/// The result a matching `result` call gets, so a follow-up finds no call left open.
const RETURNED: &str = "The result was returned.";

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
    /// `--schema`: the `result` call carries data that matches it.
    pub schema: Option<&'a Value>,
    /// What reaches the run between its turns; `None` for nothing.
    pub inbox: Option<&'a dyn Inbox>,
    /// The model's context window in tokens (0 when it is not known): the strategy fits the
    /// conversation to the limit it leaves.
    pub window: u64,
    /// How the conversation is prepared for the model before each request.
    pub strategy: &'a dyn Strategy,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// The model answered: a message with no call.
    Done,
    /// Turns ran out before an answer.
    Exhausted,
    /// The model, or what a tool depends on, could not be reached.
    Error(String),
}

pub struct Run {
    pub status: Status,
    pub answer: String,
    /// `--schema`: the data, or why none matched.
    pub data: Option<Result<Value, String>>,
    pub spend: Spend,
    /// The conversation as it ended: a follow-up goal goes on from it.
    pub conversation: Conversation,
    /// The tokens the conversation took at its last turn.
    pub used: u64,
}

impl Agent<'_> {
    /// Reach `goal` in a new conversation, which opens with the context.
    pub fn run(&self, goal: &str) -> Run {
        self.run_with(goal, &[])
    }

    /// Reach `goal` in `conversation`, after what was done there before. The context is not
    /// repeated: the conversation already opens with it.
    pub fn follow_up(&self, conversation: Conversation, goal: &str) -> Run {
        self.follow_up_with(conversation, goal, &[])
    }

    /// `run`, with the pictures the person attached to the goal, each with its name.
    pub fn run_with(&self, goal: &str, attached: &[(String, Image)]) -> Run {
        self.reach(Self::attach(self.tell(None, &Self::goal(goal)), attached))
    }

    /// `follow_up`, with the pictures the person attached to the goal.
    pub fn follow_up_with(
        &self,
        conversation: Conversation,
        goal: &str,
        attached: &[(String, Image)],
    ) -> Run {
        self.reach(Self::attach(
            self.tell(Some(conversation), &Self::goal(goal)),
            attached,
        ))
    }

    fn attach(mut conversation: Conversation, attached: &[(String, Image)]) -> Conversation {
        for (name, image) in attached {
            conversation.attach(name, image.clone());
        }
        conversation
    }

    /// Say `message` at the end of `conversation` (its open calls closed first), or open a new one
    /// with it after the context: a goal, or something the app says between goals (a setting).
    ///
    /// The context is a message of its own, before the first goal: it is the same for every task
    /// and session in a workspace, so the prompt cache keeps it, while each goal comes after it.
    pub fn tell(&self, conversation: Option<Conversation>, message: &str) -> Conversation {
        let mut conversation = conversation.unwrap_or_else(|| {
            let mut opened = Conversation::new(SYSTEM);
            let context = self.context.trim();
            if !context.is_empty() {
                opened.push(Message::User(format!("<context>\n{context}\n</context>")));
            }
            opened
        });
        conversation.close_open_calls();
        conversation.push(Message::User(message.to_string()));
        conversation
    }

    fn goal(goal: &str) -> String {
        format!("Goal: {goal}")
    }

    fn reach(&self, conversation: Conversation) -> Run {
        let specs: Vec<ToolSpec> = self.tools.iter().map(|tool| tool.spec()).collect();
        let mut run = Run {
            status: Status::Exhausted,
            answer: String::new(),
            data: None,
            spend: Spend::default(),
            conversation,
            used: 0,
        };
        // Real tokens per estimated one, from the last answer's usage.
        let mut ratio = 1.0;
        // The specs go with every request: they count in the usage the ratio is taken from, and
        // against the limit, but no stage can elide them.
        let fixed: u64 = specs
            .iter()
            .map(|spec| {
                let size = spec.name.len() + spec.description.len();
                (size + spec.input.schema().to_string().len()) as u64 / 4
            })
            .sum();
        for turn in 0..self.max_turns {
            Self::deliver(&mut run, self.take());
            // Prepared for the model before each request; the elided stays elided.
            let ceiling = self
                .strategy
                .ceiling(self.window)
                .saturating_sub(whole(fixed as f64 * ratio));
            let report = self.strategy.prepare(&mut run.conversation, ceiling, ratio);
            if !report.is_empty() {
                self.observer.fitted(&report);
            }
            let estimate = run.conversation.tokens();
            let request = Request {
                conversation: run.conversation.messages(),
                tools: &specs,
                calling: Calling::Any,
                deadline: None,
            };
            let answered = self.chat.complete(request);
            let Some(completion) = self.kept(answered, &mut run) else {
                return run;
            };
            if completion.usage.input_tokens > 0 && estimate > 0 {
                let input = completion.usage.input_tokens as f64;
                ratio = (input / (estimate + fixed) as f64).clamp(0.3, 4.0);
            }
            // An answer waits for what is still to come (not on the last turn: then it stands).
            if turn + 1 < self.max_turns && self.awaited(&completion, &mut run) {
                continue;
            }
            if completion.calls.is_empty() {
                // A message with no call is the answer: the task ends with it, as a person's
                // final message ends their turn.
                run.status = Status::Done;
                run.answer = completion.text.unwrap_or_default().trim().to_string();
                self.conclude(&mut run);
                return run;
            }
            self.say(&completion);
            if self.handle_all(&completion.calls, &specs, &mut run) {
                return run;
            }
        }
        run
    }

    /// A request's answer counted and kept in the conversation. `None` when the model could not
    /// be reached: the run has ended with the error.
    fn kept(&self, answered: Result<Completion, ChatError>, run: &mut Run) -> Option<Completion> {
        let completion = match answered {
            Ok(completion) => completion,
            Err(error) => {
                run.status = Status::Error(error.to_string());
                return None;
            }
        };
        self.observer.turn(&completion.usage);
        run.spend.add(&completion.usage);
        run.used = completion.usage.input_tokens + completion.usage.output_tokens;
        if completion.calls.is_empty() && completion.text.as_deref().is_none_or(str::is_empty) {
            // Nothing said and nothing called: kept, it is a message some hosts reject.
            self.observer.said("");
        } else {
            run.conversation.push(completion.message());
        }
        Some(completion)
    }

    /// With `--schema`, once the work has ended: the `result` call asked for and forced, its data
    /// checked; a mismatch goes back to be fixed, until the fixes run out and the run has none.
    fn conclude(&self, run: &mut Run) {
        let Some(schema) = self.schema else {
            return;
        };
        let specs = [schema::spec(schema)];
        run.conversation.push(Message::User(DATA_NOW.into()));
        let mut problems = String::from("no `result` call came");
        for _ in 0..=MAX_FIXES {
            let request = Request {
                conversation: run.conversation.messages(),
                tools: &specs,
                calling: Calling::Only(schema::TOOL),
                deadline: None,
            };
            let answered = self.chat.complete(request);
            let Some(completion) = self.kept(answered, run) else {
                return;
            };
            let Some(call) = (completion.calls.iter()).find(|call| call.name == schema::TOOL)
            else {
                break;
            };
            self.observer.call(call);
            match schema::take(&call.arguments, schema) {
                Ok(data) => {
                    let outcome = Ok(ToolOutput::from(RETURNED.to_string()));
                    self.observer.result(call, &outcome);
                    run.conversation
                        .result(&call.id, ToolOutput::from(RETURNED.to_string()));
                    run.data = Some(Ok(data));
                    return;
                }
                Err(why) => {
                    let said = format!("The result is not taken: {why}.");
                    let outcome = Err(ToolError::Failed(said.clone()));
                    self.observer.result(call, &outcome);
                    run.conversation.failure(&call.id, &said);
                    problems = why;
                }
            }
        }
        run.data = Some(Err(format!(
            "the result did not match the schema: {problems}"
        )));
    }

    /// Whether an answer (a message with no call) was held for what reached the run meanwhile,
    /// or is still to come: then it was delivered and the run goes on. Whether more is to come is
    /// asked before taking: a source that is done posts its last entry before it says so, so
    /// nothing it sent is left behind. On the last turn the answer stands (the caller skips
    /// this), and what is still to come waits for the next goal.
    fn awaited(&self, completion: &Completion, run: &mut Run) -> bool {
        if !completion.calls.is_empty() {
            return false;
        }
        let expecting = self.expecting();
        let came = self.take();
        let waits = came.is_empty() && expecting;
        if waits || !came.is_empty() {
            self.say(completion);
        }
        let came = match waits {
            true => self.inbox.map(|inbox| inbox.wait()).unwrap_or_default(),
            false => came,
        };
        if came.is_empty() {
            return false;
        }
        Self::deliver(run, came);
        true
    }

    /// Show what the model said in a turn the run goes on from: it is not the answer.
    fn say(&self, completion: &Completion) {
        let text = completion.text.as_deref().map(str::trim);
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            self.observer.said(text);
        }
    }

    fn take(&self) -> Vec<String> {
        self.inbox.map(|inbox| inbox.take()).unwrap_or_default()
    }

    fn expecting(&self) -> bool {
        self.inbox.is_some_and(|inbox| inbox.expecting())
    }

    /// Say what came in, after the results of the last turn's calls.
    fn deliver(run: &mut Run, came: Vec<String>) {
        for message in came {
            run.conversation.push(Message::User(message));
        }
    }
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
