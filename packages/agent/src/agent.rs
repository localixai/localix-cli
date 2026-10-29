//! The agent's loop: the chat model reads the goal and calls tools until it calls `result`
//! ([`crate::schema`]), bounded by `max_turns` (unlimited unless the app sets one). A reply
//! without a call is a note, not the end: the model is told that only `result` ends the task,
//! each time, until it calls it or its turns (or the app's time) run out. Nothing else pushes the
//! model on: a failed call goes back to it with its reason, and on the last of its turns it is
//! asked for `result`.
//! The calls of one turn that may run together do ([`crate::calls`]). Before each turn, what
//! came into the inbox is said; an answer while something is still to come waits for it, and the
//! run goes on.

mod handling;

use contracts::chat::{Chat, Completion, Message, Request, Spend};
use contracts::inbox::Inbox;
use contracts::tool::{Image, Tool, ToolError, ToolSpec};
use serde_json::Value;

use crate::conversation::Conversation;
use crate::history::{Strategy, whole};
use crate::observer::Observer;
use crate::schema;

const SYSTEM: &str = include_str!("prompt.md");
/// Times a `result` that breaks its schema goes back to be fixed.
const MAX_FIXES: usize = 2;
/// Said when a reply ends without a call: text does not end the task.
const RESULT_NOW: &str = "Text does not end the task. Go on with the work, and call `result` \
    when the goal is reached or something outside your reach blocks it.";
/// The result a matching `result` call gets, so a follow-up finds no call left open.
const RETURNED: &str = "The result was returned.";
/// A `result` answered while an agent's answer or a job's lines are still to come.
const NOT_YET: &str = "Not taken: something you started is still to come. Read it, then call \
    `result` again.";

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
    /// `result` said the goal was reached.
    Achieved,
    /// `result` said it was not, and what blocks it.
    NotAchieved,
    /// Turns ran out before `result`.
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

/// What one goal has used of its second chances.
#[derive(Default)]
struct Tally {
    /// `result` calls sent back to be fixed.
    fixes: usize,
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
        let mut specs: Vec<ToolSpec> = self.tools.iter().map(|tool| tool.spec()).collect();
        specs.push(schema::spec(self.schema));
        let mut run = Run {
            status: Status::Exhausted,
            answer: String::new(),
            data: None,
            spend: Spend::default(),
            conversation,
            used: 0,
        };
        let mut tally = Tally::default();
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
            let last = turn + 1 == self.max_turns;
            if last {
                Self::remind(&mut run);
            }
            let request = Request {
                conversation: run.conversation.messages(),
                tools: &specs,
                may_call: true,
                deadline: None,
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
            run.used = completion.usage.input_tokens + completion.usage.output_tokens;
            if completion.usage.input_tokens > 0 && estimate > 0 {
                let input = completion.usage.input_tokens as f64;
                ratio = (input / (estimate + fixed) as f64).clamp(0.3, 4.0);
            }
            if completion.calls.is_empty() && completion.text.as_deref().is_none_or(str::is_empty) {
                // Nothing said and nothing called: kept, it is a message some hosts reject.
                self.observer.said("");
            } else {
                run.conversation.push(completion.message());
            }
            // An answer (no call, or `result` alone) waits for what is still to come.
            if !last && self.awaited(&completion, &mut run) {
                continue;
            }
            self.say(&completion);
            if completion.calls.is_empty() {
                // Text alone is a note: the model is told, each time, that only `result` ends
                // the task. The run does not pass prose off as the result a pipeline expects;
                // its turns and the app's time limit bound it.
                Self::remind(&mut run);
                continue;
            }
            if self.handle_all(&completion.calls, &specs, &mut run, &mut tally) {
                return run;
            }
        }
        run
    }

    /// Whether an answer (no call, or `result` alone) was held for what reached the run
    /// meanwhile, or is still to come: then it was delivered and the run goes on. Whether more is
    /// to come is asked before taking: a source that is done posts its last entry before it says
    /// so, so nothing it sent is left behind. On the last turn the answer stands (the caller
    /// skips this), and what is still to come waits for the next goal.
    fn awaited(&self, completion: &Completion, run: &mut Run) -> bool {
        let ending = (completion.calls.iter()).all(|call| call.name == schema::TOOL);
        if !ending {
            return false;
        }
        let expecting = self.expecting();
        let came = self.take();
        let waits = came.is_empty() && expecting;
        if waits || !came.is_empty() {
            self.say(completion);
            // A `result` before what is still to come is not taken: it is answered.
            for call in &completion.calls {
                let early = ToolError::Failed(NOT_YET.into());
                self.observer.result(call, &Err(early));
                run.conversation.failure(&call.id, NOT_YET);
            }
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

    /// Tell the model that only `result` ends the task, unless it was just told.
    fn remind(run: &mut Run) {
        let told = matches!(run.conversation.messages().last(),
            Some(Message::User(text)) if text == RESULT_NOW);
        if !told {
            run.conversation.push(Message::User(RESULT_NOW.into()));
        }
    }
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
