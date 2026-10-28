use contracts::chat::{Completion, ToolCall};
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
        assert_eq!(names(rig), ["bash", "ask", "agents", "jobs"]);
        0
    });
    composed(crate::modes::built("bypass"), false, vec![], |rig| {
        assert_eq!(names(rig), ["bash", "agents", "jobs"]);
        0
    });
}

#[test]
fn status_shows_the_mode() {
    composed(crate::modes::built("plan"), true, vec![], |rig| {
        let status = crate::commands::repl::status(rig);
        assert!(status.contains("mode       plan\n"), "{status}");
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
    use crate::testing::{Captured, Scratch, ScriptedChat};
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
    };
    let chat = ScriptedChat::new(vec![]);
    let models = Models {
        chat: &chat,
        live: None,
    };
    let output = Output::with(
        false,
        false,
        Box::new(Captured::default()),
        Box::new(Captured::default()),
    );
    let (shared, seat) = setup.split();
    assemble(&shared, seat, models, &output, &Interrupt::new(), |rig| {
        let hidden = (rig.jobs_kit.hidden)();
        assert!(hidden.contains(&"DEEPSEEK_API_KEY".into()), "{hidden:?}");
        assert!(hidden.contains(&"OPENROUTER_API_KEY".into()), "{hidden:?}");
        0
    });
}

/// In `plan` the model's commands run in the sandbox: one reads, its write fails and the file
/// stays; in `bypass` the same command changes it.
#[test]
fn plan_confines_the_models_commands_to_reading() {
    for (mode, changed) in [("bypass", true), ("plan", false)] {
        let call = ToolCall {
            id: "c1".into(),
            name: "bash".into(),
            arguments: json!({"command": "cat a.txt && echo two > a.txt"}),
        };
        let mut replies = vec![Completion {
            calls: vec![call],
            ..Completion::default()
        }];
        replies.extend(answers(1));
        composed(crate::modes::built(mode), true, replies, |rig| {
            let root = rig.workspace.root();
            std::fs::write(root.join("a.txt"), "one\n").unwrap();
            rig.task("change a");
            let results = crate::testing::results(rig);
            let report = &results[0].1;
            assert!(report.contains("one"), "{mode}: {report}");
            let now = std::fs::read_to_string(root.join("a.txt")).unwrap();
            assert_eq!(now == "two\n", changed, "{mode}: {report}");
            0
        });
    }
}

/// `exec`'s calls are the main session's own: a subtree's instructions come once, as for a
/// direct call (ARCHITECTURE.md X2). Files change through `apply_patch` alone, never a script.
/// A provider chosen with `/login` after the session began has its key kept from commands too:
/// the variables are read at each command, not fixed when the session starts.
#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the key must be in the process's environment, where the commands inherit it"
)]
fn a_provider_chosen_later_has_its_key_kept_from_commands_too() {
    // SAFETY: no other test reads or sets this variable.
    unsafe { std::env::set_var("CEREBRAS_API_KEY", "csk-test") };
    let probe = || Completion {
        calls: vec![crate::testing::scripted(
            "b",
            "bash",
            json!({"command": "echo key=${CEREBRAS_API_KEY:-none}"}),
        )],
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
fn two_lanes_over_one_shared_share_the_workspace_but_keep_their_own() {
    use crate::testing::{Scratch, ScriptedChat, quiet, setup};
    let scratch = Scratch::new();
    let (shared, seat) = setup(&scratch, crate::modes::built("bypass"), true).split();
    let other = shared.seat(shared.history.start(), None);
    let chat = ScriptedChat::new(vec![]);
    let models = || Models {
        chat: &chat,
        live: None,
    };
    let output = quiet();
    let names =
        |rig: &Rig| -> Vec<String> { rig.agent.tools.iter().map(|t| t.spec().name).collect() };
    assemble(&shared, seat, models(), &output, &Interrupt::new(), |a| {
        assemble(&shared, other, models(), &output, &Interrupt::new(), |b| {
            assert!(std::ptr::eq(a.workspace, b.workspace));
            assert!(std::ptr::eq(a.history, b.history));
            assert!(!std::ptr::eq(a.checkpoints, b.checkpoints));
            assert!(
                !std::ptr::addr_eq(a.agents, b.agents),
                "agents are a lane's own"
            );
            assert!(!std::ptr::eq(a.jobs, b.jobs));
            assert_ne!(a.session.id, b.session.id);
            assert_eq!(names(a), names(b));
            assert_eq!(names(b), ["bash", "ask", "agents", "jobs"]);
            0
        })
    });
}
