use contracts::chat::{Completion, Message, ToolCall};
use serde_json::json;

use super::*;
use crate::mode::PLAN_REFUSAL;
use crate::testing::{answers, composed, users};

#[test]
fn a_mode_change_is_told_once_before_the_next_task() {
    composed(Mode::Default, true, answers(3), |rig| {
        rig.task("one");
        rig.switch(Mode::Plan);
        rig.task("two");
        rig.task("three");
        let users = users(rig);
        assert_eq!(users.len(), 4, "{users:?}");
        assert_eq!(users[0], "Goal: one");
        assert!(users[1].starts_with("<mode_change>\nThe mode is now plan: "));
        assert_eq!(users[2..], ["Goal: two", "Goal: three"]);
        0
    });
}

#[test]
fn a_new_conversation_states_the_mode_it_starts_in() {
    composed(Mode::Default, true, answers(2), |rig| {
        rig.switch(Mode::Bypass);
        rig.task("one");
        assert!(users(rig)[0].starts_with("<mode_change>\nThe mode is now bypass"));
        rig.start_over();
        rig.task("two");
        let users = users(rig);
        assert_eq!(
            users.len(),
            2,
            "the fresh context is followed by the note: {users:?}"
        );
        assert!(users[0].contains("The mode is now bypass"));
        assert_eq!(users[1], "Goal: two");
        0
    });
}

#[test]
fn entering_plan_remembers_the_mode_it_came_from() {
    composed(Mode::Bypass, true, vec![], |rig| {
        assert_eq!(rig.before_plan, Mode::Bypass);
        rig.switch(Mode::Autonomous);
        rig.switch(Mode::Plan);
        rig.switch(Mode::Plan);
        assert_eq!(rig.before_plan, Mode::Autonomous);
        assert_eq!(rig.mode.get(), Mode::Plan);
        0
    });
    composed(Mode::Plan, true, vec![], |rig| {
        assert_eq!(
            rig.before_plan,
            Mode::Default,
            "llx --mode plan carries out in default"
        );
        0
    });
}

#[test]
fn ask_is_offered_in_a_session_only() {
    let names = |rig: &Rig| -> Vec<String> {
        rig.agent
            .tools
            .iter()
            .map(|tool| tool.spec().name)
            .collect()
    };
    composed(Mode::Default, true, vec![], |rig| {
        assert_eq!(names(rig), ["read", "edit", "bash", "ask"]);
        0
    });
    composed(Mode::Autonomous, false, vec![], |rig| {
        assert_eq!(names(rig), ["read", "edit", "bash"]);
        0
    });
}

#[test]
fn plan_mode_changes_no_file() {
    let create = ToolCall {
        id: "c1".into(),
        name: "edit".into(),
        arguments: json!({"path": "plan.md", "old_string": "", "new_string": "x"}),
    };
    let mut replies = vec![Completion {
        calls: vec![create],
        ..Completion::default()
    }];
    replies.extend(answers(1));
    composed(Mode::Plan, true, replies, |rig| {
        rig.task("write the plan to plan.md");
        let refused = rig
            .conversation
            .as_ref()
            .unwrap()
            .messages()
            .iter()
            .any(|message| {
                matches!(message, Message::ToolResult { content, failed: true, .. }
                if content.contains(PLAN_REFUSAL))
            });
        assert!(refused);
        assert!(!rig.workspace.root().join("plan.md").exists());
        0
    });
}

#[cfg(unix)]
#[test]
fn a_terminal_session_ends_with_its_task() {
    let start = ToolCall {
        id: "c1".into(),
        name: "bash".into(),
        arguments: json!({"command": "sleep 30", "tty": true, "yield_ms": 100}),
    };
    let mut replies = vec![Completion {
        calls: vec![start],
        ..Completion::default()
    }];
    replies.extend(answers(1));
    composed(Mode::Bypass, false, replies, |rig| {
        rig.task("start a sleep in a terminal");
        let started = rig
            .conversation
            .as_ref()
            .unwrap()
            .messages()
            .iter()
            .any(|message| {
                matches!(message, Message::ToolResult { content, failed: false, .. }
                if content.starts_with("session 1 running (`sleep 30`)"))
            });
        assert!(
            started,
            "{:?}",
            rig.conversation.as_ref().unwrap().messages()
        );
        assert_eq!(rig.sessions.running(), "none");
        0
    });
}

#[cfg(unix)]
#[test]
fn status_shows_the_mode_and_the_terminals_and_new_closes_them() {
    composed(Mode::Plan, true, vec![], |rig| {
        let wait = tools::shell::Wait {
            needle: None,
            yield_ms: Some(100),
        };
        let root = rig.workspace.root().to_path_buf();
        let turn = rig
            .sessions
            .start("sleep 30", &root, &[], wait, &|| false)
            .unwrap();
        assert_eq!(turn.id, 1);
        let status = crate::commands::repl::status(rig);
        assert!(status.contains("mode       plan\n"), "{status}");
        assert!(status.contains("terminals  1 (`sleep 30`)\n"), "{status}");
        rig.start_over();
        let status = crate::commands::repl::status(rig);
        assert!(status.contains("terminals  none\n"), "{status}");
        0
    });
}
