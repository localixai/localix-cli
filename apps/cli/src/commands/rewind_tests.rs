use std::collections::VecDeque;

use contracts::chat::{Chat, ChatError, Completion, Message, Request, ToolCall};
use serde_json::json;

use super::*;
use crate::mode::Mode;
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

fn edit(arguments: serde_json::Value) -> Completion {
    Completion {
        calls: vec![ToolCall {
            id: "e".into(),
            name: "edit".into(),
            arguments,
        }],
        ..Completion::default()
    }
}

#[test]
fn rewinding_puts_back_the_files_and_the_conversation_before_the_task() {
    let mut replies = vec![edit(
        json!({"path": "a.txt", "old_string": "", "new_string": "1\n"}),
    )];
    replies.extend(answers(1));
    replies.push(edit(
        json!({"path": "a.txt", "old_string": "1", "new_string": "2"}),
    ));
    replies.extend(answers(1));
    composed(Mode::Bypass, true, replies, |rig| {
        rig.task("make a");
        let after_first = rig.conversation.as_ref().unwrap().messages().len();
        rig.task("change a");
        let file = rig.workspace.root().join("a.txt");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "2\n");
        // The newest task first: back to before "change a", code and conversation.
        let said = steps(rig, &mut picks(&[Some(0), Some(0)]));
        assert!(said.starts_with("Rewound: 1 file(s) put back"), "{said}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "1\n");
        assert_eq!(
            rig.conversation.as_ref().unwrap().messages().len(),
            after_first
        );
        // Before the first, the conversation only: the file stays, the conversation is new.
        let said = steps(rig, &mut picks(&[Some(0), Some(1)]));
        assert!(said.contains("the conversation is back"), "{said}");
        assert!(file.exists() && rig.conversation.is_none());
        assert!(steps(rig, &mut picks(&[])).starts_with("Nothing to rewind"));
        0
    });
}

#[test]
fn background_tasks_are_listed_as_the_transcript_shows_them_and_not_put_back_in_the_prompt() {
    composed(Mode::Bypass, true, answers(2), |rig| {
        rig.task("(scheduled `hello`) say hello");
        rig.task(&crate::arrivals::monitor("ticks", "tick 1").render());
        let (ui, events) = crate::tui::test_support::fake_ui();
        rig.ui = Some(ui);
        let mut person = picks(&[Some(0), Some(1)]);
        steps(rig, &mut person);
        assert_eq!(person.1[0], ["≋ ticks · tick 1", "◷ hello · say hello"]);
        let prefilled = events
            .try_iter()
            .any(|event| matches!(event, Event::Prefill(_)));
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
fn rewinding_the_conversation_lets_its_agents_go() {
    composed_with(Mode::Bypass, true, &Delegates, |rig| {
        rig.task("write it with an agent");
        steps(rig, &mut picks(&[Some(0), Some(0)]));
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
    composed(Mode::Bypass, true, answers(1), |rig| {
        rig.task(crate::commands::extras::INIT);
        let mut person = picks(&[None]);
        steps(rig, &mut person);
        assert_eq!(person.1[0], ["/init"]);
        0
    });
}

#[test]
fn rewinding_while_another_session_works_here_warns() {
    let mut replies = vec![edit(
        json!({"path": "a.txt", "old_string": "", "new_string": "1\n"}),
    )];
    replies.extend(answers(1));
    composed(Mode::Bypass, true, replies, |rig| {
        rig.task("make a");
        let said = steps(rig, &mut picks(&[None]));
        assert!(!said.contains("Another session"), "{said}");
        // Another lane over the same workspace is in a task.
        rig.shared
            .working
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let said = steps(rig, &mut picks(&[Some(0), Some(0)]));
        assert!(
            said.starts_with(
                "Another session is working in this directory; restoring may undo its changes."
            ),
            "{said}"
        );
        0
    });
}
