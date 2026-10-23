//! The agent's loop: the chat model reads the goal, calls tools until it answers, bounded by
//! turns. A reply without a tool call is the answer. Near the end of its turns, or after calls keep
//! failing, the model is asked to answer with what it has. The calls of one turn that may run
//! together do ([`crate::calls`]). Before each turn, what came into the inbox is said; an answer
//! while something is still to come waits for it, and the run goes on.

mod handling;

use contracts::chat::{Chat, Completion, Message, Request};
use contracts::inbox::Inbox;
use contracts::tool::{Image, Tool, ToolSpec};
use serde_json::Value;

use crate::answer;
use crate::conversation::Conversation;
use crate::history::Strategy;
use crate::observer::Observer;
use crate::schema;
pub use crate::spend::Spend;

const SYSTEM: &str = include_str!("prompt.md");
/// Turns whose calls all failed, in a row, before the model is asked to answer.
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
    /// Turns whose calls all failed, in a row.
    failures: usize,
    /// Whether a call of this turn failed, and whether one succeeded.
    failed: bool,
    succeeded: bool,
    /// `result` calls sent back to be fixed.
    fixes: usize,
    /// Whether the model was told to answer now: it is told once.
    reminded: bool,
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
        self.reach(self.attach(self.tell(None, &self.goal(goal)), attached))
    }

    /// `follow_up`, with the pictures the person attached to the goal.
    pub fn follow_up_with(
        &self,
        conversation: Conversation,
        goal: &str,
        attached: &[(String, Image)],
    ) -> Run {
        self.reach(self.attach(self.tell(Some(conversation), &self.goal(goal)), attached))
    }

    fn attach(&self, mut conversation: Conversation, attached: &[(String, Image)]) -> Conversation {
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

    /// The goal as the model reads it, with what a schema asks of the end.
    fn goal(&self, goal: &str) -> String {
        match self.schema {
            Some(_) => format!("Goal: {goal}\n\n{}", schema::BRIEF),
            None => format!("Goal: {goal}"),
        }
    }

    fn reach(&self, conversation: Conversation) -> Run {
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
                (size + spec.parameters.to_string().len()) as u64 / 4
            })
            .sum();
        for turn in 0..self.max_turns {
            self.deliver(&mut run, self.take());
            // Prepared for the model before each request; the elided stays elided.
            let ceiling =
                (self.strategy.ceiling(self.window)).saturating_sub((fixed as f64 * ratio) as u64);
            let report = self.strategy.prepare(&mut run.conversation, ceiling, ratio);
            if !report.is_empty() {
                self.observer.fitted(&report);
            }
            let estimate = run.conversation.tokens();
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
            run.used = completion.usage.input_tokens + completion.usage.output_tokens;
            if completion.usage.input_tokens > 0 && estimate > 0 {
                let input = completion.usage.input_tokens as f64;
                ratio = (input / (estimate + fixed) as f64).clamp(0.3, 4.0);
            }
            run.conversation.push(completion.message());
            if completion.calls.is_empty() {
                // Something came in meanwhile, or is still to come: the run goes on with it. On
                // the last turn the answer stands, and what is still to come waits for the next goal.
                let last = turn + 1 == self.max_turns;
                // Whether more is to come is asked before taking: a source that is done posts
                // its last entry before it says so, so nothing it sent is left behind.
                let expecting = !last && self.expecting();
                let came = if last { vec![] } else { self.take() };
                let waits = came.is_empty() && expecting;
                if waits || !came.is_empty() {
                    self.say(&completion);
                }
                let came = match waits {
                    true => self.inbox.map(|inbox| inbox.wait()).unwrap_or_default(),
                    false => came,
                };
                if !came.is_empty() {
                    self.deliver(&mut run, came);
                    continue;
                }
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
            self.say(&completion);
            if self.handle_all(&completion.calls, &specs, &mut run, &mut tally) {
                return run;
            }
        }
        run
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
    fn deliver(&self, run: &mut Run, came: Vec<String>) {
        for message in came {
            run.conversation.push(Message::User(message));
        }
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
