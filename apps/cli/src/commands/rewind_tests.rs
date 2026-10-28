use std::collections::VecDeque;

use contracts::chat::{Chat, ChatError, Completion, Message, Request, ToolCall};
use serde_json::json;

use super::*;
use crate::picker::{Lines, Picker};
use crate::testing::{answers, composed, composed_with};

/// A person who picks from a script; the labels of each list shown.
struct Picks(VecDeque<Option<usize>>, Vec<Vec<String>>);

impl Picker for Picks {
    fn pick(&mut self, _: &str, items: &[Item]) -> Option<usize> {
        self.1
            .push(items.iter().map(|item| item.label.clone()).collect());
        self.0.pop_front().flatten()
    }
}

fn picks(script: &[Option<usize>]) -> Picks {
    Picks(script.iter().copied().collect(), vec![])
}

impl Lines for Picks {
    fn line(&mut self, _: &str, _: bool) -> Option<String> {
        None
    }
}

fn change(command: &str) -> Completion {
    Completion {
        calls: vec![crate::testing::scripted(
            "b",
            "bash",
            json!({"command": command}),
        )],
        ..Completion::default()
    }
}

#[test]
fn rewinding_goes_back_in_the_conversation_and_leaves_the_files() {
    let mut replies = vec![change("echo 1 > a.txt")];
    replies.extend(answers(1));
    replies.push(change("echo 2 > a.txt"));
    replies.extend(answers(1));
    composed(crate::modes::built("bypass"), true, replies, |rig| {
        rig.task("make a");
        let after_first = rig.conversation.as_ref().unwrap().messages().len();
        rig.task("change a");
        let file = rig.workspace.root().join("a.txt");
        // The newest task first: back to before "change a".
        let said = steps(rig, &mut picks(&[Some(0)]));
        assert!(said.contains("the conversation is back"), "{said}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "2\n");
        assert_eq!(
            rig.conversation.as_ref().unwrap().messages().len(),
            after_first
        );
        // Before the first: the conversation is new.
        steps(rig, &mut picks(&[Some(0)]));
        assert!(rig.conversation.is_none());
        assert!(steps(rig, &mut picks(&[])).starts_with("Nothing to rewind"));
        0
    });
}

#[test]
fn background_tasks_are_listed_as_the_transcript_shows_them_and_not_put_back_in_the_prompt() {
    composed(crate::modes::built("bypass"), true, answers(2), |rig| {
        rig.task("(scheduled `hello`) say hello");
        rig.task(&crate::arrivals::monitor("ticks", "tick 1").render());
        let (ui, frames) = crate::testing::fake_ui();
        rig.ui = Some(ui);
        let mut person = picks(&[Some(1)]);
        steps(rig, &mut person);
        assert_eq!(person.1[0], ["≋ ticks · tick 1", "◷ hello · say hello"]);
        let prefilled = (crate::testing::events(&frames).into_iter())
            .any(|(_, event)| matches!(event, Event::Prefill(_)));
        assert!(!prefilled);
        0
    });
}

/// A main agent that starts `writer` for each new task and is done once it hears back; the
/// agent is done at once.
struct Delegates;

impl Chat for Delegates {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        let user = |m: &Message, start: &str| matches!(m, Message::User(t) if t.starts_with(start));
        let agent = request.conversation.iter().any(|m| user(m, "You are `"));
        let new_task = request
            .conversation
            .last()
            .is_some_and(|m| user(m, "Goal:"));
        Ok(match (agent, new_task) {
            (false, true) => Completion {
                calls: vec![ToolCall {
                    id: "a".into(),
                    name: "agents".into(),
                    arguments: json!({"action": "spawn", "task": "write it", "name": "writer"}),
                }],
                ..Completion::default()
            },
            _ => answers(1).remove(0),
        })
    }
}

#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn rewinding_the_conversation_lets_its_agents_go() {
    composed_with(crate::modes::built("bypass"), true, &Delegates, |rig| {
        rig.task("write it with an agent");
        steps(rig, &mut picks(&[Some(0)]));
        rig.task("write it with an agent");
        let started = rig
            .conversation
            .as_ref()
            .unwrap()
            .messages()
            .iter()
            .find_map(|m| match m {
                Message::ToolResult { content, .. } => Some(content.clone()),
                _ => None,
            })
            .unwrap();
        assert!(
            started.starts_with("Started `writer` (session"),
            "{started}"
        );
        0
    });
}

#[test]
fn init_is_listed_as_the_command_typed_not_its_instruction() {
    composed(crate::modes::built("bypass"), true, answers(1), |rig| {
        rig.task(crate::commands::extras::INIT);
        let mut person = picks(&[None]);
        steps(rig, &mut person);
        assert_eq!(person.1[0], ["/init"]);
        0
    });
}
