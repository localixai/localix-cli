use contracts::chat::{Completion, Message, ToolCall};
use serde_json::json;
use tools::workspace::Workspace;

use contracts::history::History;

use super::*;
use crate::choice::Choice;
use crate::testing::{answers, composed, users};

#[test]
fn a_mode_change_is_told_once_before_the_next_task() {
    composed(crate::modes::built("default"), true, answers(3), |rig| {
        rig.task("one");
        rig.switch(crate::modes::built("plan"));
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
fn ask_is_offered_in_a_session_only() {
    let names = |rig: &Rig| -> Vec<String> {
        rig.agent
            .tools
            .iter()
            .map(|tool| tool.spec().name)
            .collect()
    };
    composed(crate::modes::built("default"), true, vec![], |rig| {
        assert_eq!(
            names(rig),
            ["read", "edit", "bash", "window", "ask", "agents", "jobs"]
        );
        0
    });
    composed(crate::modes::built("bypass"), false, vec![], |rig| {
        assert_eq!(
            names(rig),
            ["read", "edit", "bash", "window", "agents", "jobs"]
        );
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
    composed(crate::modes::built("plan"), true, replies, |rig| {
        rig.task("write the plan to plan.md");
        let refused = rig
            .conversation
            .as_ref()
            .unwrap()
            .messages()
            .iter()
            .any(|message| {
                matches!(message, Message::ToolResult { content, failed: true, .. }
                if content.contains("refused by the rule `edit`"))
            });
        assert!(refused);
        assert!(!rig.workspace.root().join("plan.md").exists());
        0
    });
}

/// A task that starts `sleep 30` in a terminal of its own.
#[cfg(unix)]
fn a_terminal_started() -> Vec<Completion> {
    let start = ToolCall {
        id: "c1".into(),
        name: "window".into(),
        arguments: json!({"action": "open", "kind": "terminal", "command": "sleep 30",
            "yield_ms": 100}),
    };
    let mut replies = vec![Completion {
        calls: vec![start],
        ..Completion::default()
    }];
    replies.extend(answers(1));
    replies
}

#[cfg(unix)]
#[test]
fn a_terminal_session_outlives_its_task_in_a_session() {
    let mut replies = a_terminal_started();
    // The next task reads it, by the id the first was given.
    let read = ToolCall {
        id: "c2".into(),
        name: "window".into(),
        arguments: json!({"action": "interact", "tab": "t1", "yield_ms": 50}),
    };
    replies.push(Completion {
        calls: vec![read],
        ..Completion::default()
    });
    replies.extend(answers(1));
    composed(crate::modes::built("bypass"), true, replies, |rig| {
        rig.task("start a sleep in a terminal");
        assert!(
            rig.sessions.running().contains("sleep 30"),
            "{}",
            rig.sessions.running()
        );
        rig.task("look at it again");
        let read = rig.conversation.as_ref().unwrap().messages().iter().any(|message| {
            matches!(message, Message::ToolResult { id, failed: false, .. } if id == "c2")
        });
        assert!(read, "{:?}", rig.conversation.as_ref().unwrap().messages());
        0
    });
}

#[cfg(unix)]
#[test]
fn a_terminal_session_ends_with_an_exec_task() {
    composed(
        crate::modes::built("bypass"),
        false,
        a_terminal_started(),
        |rig| {
            rig.task("start a sleep in a terminal");
            let started = rig
                .conversation
                .as_ref()
                .unwrap()
                .messages()
                .iter()
                .any(|message| {
                    matches!(message, Message::ToolResult { content, failed: false, .. }
                if content.starts_with("t1 (terminal): session 1 running (`sleep 30`)"))
                });
            assert!(
                started,
                "{:?}",
                rig.conversation.as_ref().unwrap().messages()
            );
            assert_eq!(rig.sessions.running(), "none");
            0
        },
    );
}

#[cfg(unix)]
#[test]
fn status_shows_the_mode_and_the_terminals() {
    composed(crate::modes::built("plan"), true, vec![], |rig| {
        let wait = tools::shell::Wait {
            needle: None,
            yield_ms: Some(100),
        };
        let root = rig.workspace.root().to_path_buf();
        let turn = rig
            .sessions
            .start("sleep 30", &root, Default::default(), wait, None, &|| false)
            .unwrap();
        assert_eq!(turn.id, 1);
        let status = crate::commands::repl::status(rig);
        assert!(status.contains("mode       plan\n"), "{status}");
        assert!(status.contains("terminals  1 (`sleep 30`)\n"), "{status}");
        rig.sessions.close_all();
        0
    });
}

#[test]
fn a_session_that_panics_ends_its_background_work_and_does_not_hang() {
    let begun = std::time::Instant::now();
    let unwound = std::panic::catch_unwind(|| {
        composed(crate::modes::built("bypass"), true, vec![], |_| {
            panic!("a bug in a task")
        });
    });
    assert!(unwound.is_err());
    assert!(begun.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn commands_never_see_the_key_of_the_provider_in_use() {
    use crate::testing::{Captured, NoDecider, Scratch, ScriptedChat};
    let scratch = Scratch::new();
    let history = session::Sessions {
        dir: scratch.path().join("sessions"),
    };
    let session = history.start();
    let provider = crate::providers::Provider::named("deepseek").unwrap();
    let setup = Setup {
        options: <crate::cli::Cli as clap::Parser>::try_parse_from(["llx"])
            .unwrap()
            .run,
        choice: Choice::new(provider, "deepseek-flash"),
        workspace: Workspace::new(scratch.path()).unwrap(),
        context: String::new(),
        fresh: String::new(),
        schema: None,
        mode: crate::modes::built("bypass"),
        modes: crate::modes::Modes::built_in(),
        rules: crate::rules::Rules::default(),
        terminal: false,
        asks: false,
        history,
        session,
        ui: None,
        shown: false,
        #[cfg(unix)]
        theme: None,
    };
    let chat = ScriptedChat::new(vec![]);
    let models = Models {
        chat: &chat,
        decider: &NoDecider,
        live: None,
    };
    let output = Output::with(
        false,
        false,
        Box::new(Captured::default()),
        Box::new(Captured::default()),
    );
    let (shared, seat) = setup.split(None);
    assemble(&shared, seat, models, &output, &Interrupt::new(), |rig| {
        let hidden = (rig.jobs_kit.hidden)();
        assert!(hidden.contains(&"DEEPSEEK_API_KEY".into()), "{hidden:?}");
        assert!(hidden.contains(&"OPENROUTER_API_KEY".into()), "{hidden:?}");
        0
    });
}

/// The main session's `read` and `edit` learn a subdirectory's instructions once, with the first
/// call there, and an edit is kept for `/rewind`.
#[test]
fn the_main_tools_learn_a_subtrees_instructions_once_and_keep_edits() {
    let call = |id: &str, name: &str, arguments| ToolCall {
        id: id.into(),
        name: name.into(),
        arguments,
    };
    let edit = json!({"path": "sub/a.txt", "old_string": "one", "new_string": "two"});
    let mut replies = vec![Completion {
        calls: vec![call("c1", "read", json!({"path": "sub/a.txt"}))],
        ..Completion::default()
    }];
    replies.push(Completion {
        calls: vec![call("c2", "edit", edit)],
        ..Completion::default()
    });
    replies.extend(answers(1));
    composed(crate::modes::built("bypass"), true, replies, |rig| {
        let sub = rig.workspace.root().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("AGENTS.md"), "Use tabs here.").unwrap();
        std::fs::write(sub.join("a.txt"), "one\n").unwrap();
        rig.task("change a");
        let results = crate::testing::results(rig);
        assert_eq!(results.len(), 2, "{results:?}");
        assert!(results[0].1.contains("Use tabs here."), "{results:?}");
        assert!(!results[1].1.contains("Use tabs here."), "{results:?}");
        assert_eq!(std::fs::read_to_string(sub.join("a.txt")).unwrap(), "two\n");
        assert_eq!(rig.checkpoints.tasks()[0].changed(), 1);
        0
    });
}

/// A provider chosen with `/login` after the session began has its key kept from commands too:
/// the variables are read at each command, not fixed when the session starts.
#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "the key must be in the process's environment, where the commands inherit it"
)]
fn a_provider_chosen_later_has_its_key_kept_from_commands_too() {
    // SAFETY: no other test reads or sets this variable.
    unsafe { std::env::set_var("CEREBRAS_API_KEY", "csk-test") };
    let probe = || Completion {
        calls: vec![ToolCall {
            id: "b".into(),
            name: "bash".into(),
            arguments: json!({"command": "echo key=${CEREBRAS_API_KEY:-none}"}),
        }],
        ..Completion::default()
    };
    let mut replies = vec![probe()];
    replies.extend(answers(1));
    replies.push(probe());
    replies.extend(answers(1));
    composed(crate::modes::built("bypass"), false, replies, |rig| {
        rig.task("before");
        let cerebras = crate::providers::Provider::named("cerebras").unwrap();
        rig.hide_for(&cerebras);
        rig.task("after");
        let shown: Vec<String> = crate::testing::results(rig)
            .into_iter()
            .map(|(_, report)| report)
            .collect();
        assert!(shown[0].contains("key=csk-test"), "{shown:?}");
        assert!(shown[1].contains("key=none"), "{shown:?}");
        0
    });
}

#[test]
fn two_lanes_over_one_shared_share_the_workspace_and_browser_but_keep_their_own() {
    use crate::testing::{NoDecider, Scratch, ScriptedChat, quiet, setup};
    let scratch = Scratch::new();
    let (shared, seat) = setup(&scratch, crate::modes::built("bypass"), true).split(None);
    let other = shared.seat(shared.history.start(), None);
    let chat = ScriptedChat::new(vec![]);
    let models = || Models {
        chat: &chat,
        decider: &NoDecider,
        live: None,
    };
    let output = quiet();
    let names =
        |rig: &Rig| -> Vec<String> { rig.agent.tools.iter().map(|t| t.spec().name).collect() };
    assemble(&shared, seat, models(), &output, &Interrupt::new(), |a| {
        assemble(&shared, other, models(), &output, &Interrupt::new(), |b| {
            assert!(
                std::ptr::eq(a.shared, b.shared),
                "one page, one browser lock"
            );
            assert!(std::ptr::eq(a.workspace, b.workspace));
            assert!(std::ptr::eq(a.history, b.history));
            assert!(!std::ptr::eq(a.checkpoints, b.checkpoints));
            assert!(
                !std::ptr::eq(a.sessions, b.sessions),
                "shells are a lane's own"
            );
            assert!(
                !std::ptr::addr_eq(a.agents, b.agents),
                "agents are a lane's own"
            );
            assert!(!std::ptr::eq(a.jobs, b.jobs));
            assert_ne!(a.session.id, b.session.id);
            assert_eq!(names(a), names(b));
            assert_eq!(
                names(b),
                ["read", "edit", "bash", "window", "ask", "agents", "jobs"]
            );
            0
        })
    });
}
