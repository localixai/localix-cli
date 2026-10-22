//! The run: the planner reads the goal and the page, calls tools until it finishes, bounded by
//! turns. One tool call at a time, so every call sees the page the previous one left.

use std::time::Instant;

use serde_json::{Value, json};

use crate::browser::Driver;
use crate::meter::Spend;
use crate::planner::Planner;
use crate::structured;
use crate::tools::{Tools, describe_page, schemas};

const SYSTEM: &str = include_str!("prompt.md");

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// The planner finished and says the goal was reached.
    Achieved,
    /// The planner finished and says it was not.
    NotAchieved,
    /// The planner answered without saying whether the goal was reached.
    Unstated,
    /// Turns ran out before an answer.
    Exhausted,
    /// The planner could not be reached.
    Error(String),
}

/// What a run spent. The two model spends are filled from their meters when the run ends.
#[derive(Debug, Default, Clone)]
pub struct Usage {
    pub planner: Spend,
    pub decision: Spend,
    pub actions: usize,
}

pub struct RunResult {
    pub status: Status,
    pub answer: String,
    pub usage: Usage,
    pub millis: u64,
    /// The planner's conversation as sent, for a follow-up that reuses its prompt cache.
    pub messages: Vec<Value>,
    /// `--schema`: the result as JSON, or why it could not be made.
    pub data: Option<Result<Value, String>>,
}

pub struct Orchestrator<'a> {
    pub planner: &'a dyn Planner,
    pub tools: Tools<'a>,
    pub driver: &'a dyn Driver,
    pub max_turns: usize,
    /// Each planner tool call as it is made (the live trace).
    pub on_call: &'a dyn Fn(&str, &Value),
    /// `--schema`: offered as the `result` tool from the first turn.
    pub schema: Option<Value>,
}

impl Orchestrator<'_> {
    pub fn run(&self, goal: &str) -> RunResult {
        let started = Instant::now();
        let mut usage = Usage::default();
        let page = match self.driver.observe() {
            Ok(page) => describe_page(&page),
            Err(error) => format!("The page could not be read yet: {error}"),
        };
        let mut messages = vec![
            json!({"role": "system", "content": SYSTEM}),
            json!({"role": "user", "content": format!("Goal: {goal}\n\n{}The browser is open.\n{page}",
                self.schema.as_ref().map(|_| format!("{}\n\n", structured::BRIEF)).unwrap_or_default())}),
        ];
        let tools = schemas(self.schema.as_ref());
        let finish = |status, answer: String, usage: Usage, messages: Vec<Value>| RunResult {
            status,
            answer,
            usage,
            millis: started.elapsed().as_millis() as u64,
            messages,
            data: None,
        };
        let (mut reminded, mut fixes) = (false, 0);
        for _ in 0..self.max_turns {
            let reply = match self.planner.turn(&messages, &tools) {
                Ok(reply) => reply,
                Err(error) => {
                    return finish(
                        Status::Error(error.to_string()),
                        String::new(),
                        usage,
                        messages,
                    );
                }
            };
            messages.push(reply.message.clone());
            // With a schema, the run must end in a `result` call: one reminder, in the same
            // conversation, so the cached prefix holds.
            if reply.calls.is_empty() && self.schema.is_some() && !reminded {
                reminded = true;
                messages
                    .push(json!({"role": "user", "content": "Call `result` with the data now."}));
                continue;
            }
            // Like any agent: a reply without a tool call is the answer.
            if reply.calls.is_empty() {
                let (answer, achieved) = read_result(reply.text.as_deref().unwrap_or(""));
                let status = match achieved {
                    Some(true) => Status::Achieved,
                    Some(false) => Status::NotAchieved,
                    None => Status::Unstated,
                };
                return finish(status, answer, usage, messages);
            }
            for call in &reply.calls {
                (self.on_call)(&call.name, &call.arguments);
                if let (structured::TOOL, Some(schema)) = (call.name.as_str(), &self.schema) {
                    match structured::take(&call.arguments, schema) {
                        Ok((data, summary, achieved)) => {
                            let status = if achieved {
                                Status::Achieved
                            } else {
                                Status::NotAchieved
                            };
                            let mut result = finish(status, summary, usage, messages);
                            result.data = Some(Ok(data));
                            return result;
                        }
                        Err(problems) if fixes < 2 => {
                            fixes += 1;
                            messages.push(json!({"role": "tool", "tool_call_id": call.id,
                                "content": format!("The data does not match its schema: {problems}. Call `result` again with it fixed.")}));
                            continue;
                        }
                        Err(problems) => {
                            let mut result =
                                finish(Status::NotAchieved, String::new(), usage, messages);
                            result.data = Some(Err(format!(
                                "the result did not match the schema: {problems}"
                            )));
                            return result;
                        }
                    }
                }
                let outcome = self.tools.run(&call.name, &call.arguments);
                if let Some(goal) = &outcome.goal {
                    usage.actions += goal.steps.len();
                }
                // The conversation only grows, so every turn reads its whole prefix from the prompt
                // cache. Past the budget, old pages are dropped all at once: one cache miss, then
                // a stable prefix again.
                if size(&messages) + outcome.content.len() > CONTEXT_BUDGET {
                    supersede(&mut messages);
                }
                messages.push(
                    json!({"role": "tool", "tool_call_id": call.id, "content": outcome.content}),
                );
            }
        }
        finish(Status::Exhausted, String::new(), usage, messages)
    }
}

/// The answer without its closing `RESULT:` line, and what that line says.
pub fn read_result(text: &str) -> (String, Option<bool>) {
    let text = text.trim();
    let Some(at) = text.rfind("RESULT:") else {
        return (text.to_string(), None);
    };
    let verdict = text[at + "RESULT:".len()..].trim().to_lowercase();
    let achieved = if verdict.starts_with("not") {
        Some(false)
    } else if verdict.starts_with("achieved") {
        Some(true)
    } else {
        None
    };
    (text[..at].trim().to_string(), achieved)
}

/// Characters of conversation kept before old pages are dropped (about 30k tokens).
const CONTEXT_BUDGET: usize = 120_000;

fn size(messages: &[Value]) -> usize {
    messages
        .iter()
        .map(|m| m["content"].as_str().map_or(0, str::len))
        .sum()
}

/// A long run sets many sub-goals; each tool result carries a whole page. Only the newest page
/// matters, so older results keep their first lines (what happened) and drop the page they showed.
/// The conversation then grows by a few lines per call instead of a page.
fn supersede(messages: &mut [Value]) {
    for message in messages.iter_mut().filter(|m| m["role"] == "tool") {
        let Some(content) = message["content"].as_str() else {
            continue;
        };
        if content.ends_with(SUPERSEDED) {
            continue;
        }
        let cut = ["Full text, characters", "Page: "]
            .iter()
            .filter_map(|mark| content.find(mark))
            .min();
        let kept = match cut {
            Some(at) => &content[..at],
            None => content.get(..content.len().min(400)).unwrap_or(content),
        };
        message["content"] = json!(format!("{}\n{SUPERSEDED}", kept.trim_end()));
    }
}

const SUPERSEDED: &str = "(page omitted: a newer page follows)";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_closing_line_says_whether_the_goal_was_reached() {
        assert_eq!(
            read_result("Price is 112 CHF.\nRESULT: achieved"),
            ("Price is 112 CHF.".into(), Some(true))
        );
        assert_eq!(
            read_result("No flights.\n\nRESULT: not achieved"),
            ("No flights.".into(), Some(false))
        );
        assert_eq!(read_result("Just text"), ("Just text".into(), None));
    }

    #[test]
    fn older_pages_shrink_to_what_happened() {
        let mut messages = vec![
            json!({"role": "user", "content": "Goal: x\nPage: A — a"}),
            json!({"role": "tool", "content": "The decision model took 2 steps; stopped: done.\n1. CLICK\nPage: A — http://a\nVisible text: long"}),
            json!({"role": "tool", "content": "Page: B — http://b\nControls:"}),
        ];
        supersede(&mut messages);
        assert_eq!(
            messages[0]["content"], "Goal: x\nPage: A — a",
            "the goal stays whole"
        );
        assert_eq!(
            messages[1]["content"],
            format!("The decision model took 2 steps; stopped: done.\n1. CLICK\n{SUPERSEDED}")
        );
        assert_eq!(messages[2]["content"], format!("\n{SUPERSEDED}"));
        let again = messages.clone();
        supersede(&mut messages);
        assert_eq!(messages, again, "idempotent");
    }
}
