use std::process::Command;

use super::*;
use crate::testing::Scratch;

#[expect(
    clippy::disallowed_methods,
    reason = "a test's fixture: the command ends by itself"
)]
fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
}

#[test]
fn diff_shows_changes_since_the_commit_and_new_files() {
    let Some(scratch) = Scratch::outside_repository() else {
        return;
    };
    assert!(diff(scratch.path()).starts_with("Not a git repository"));
    git(scratch.path(), &["init", "-q"]);
    scratch.file("a.txt", "one\n");
    git(scratch.path(), &["add", "."]);
    git(
        scratch.path(),
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            // A person's signing (a key agent that may be locked) has no part in the fixture.
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "a",
        ],
    );
    assert_eq!(diff(scratch.path()), "No changes since the last commit.\n");
    scratch.file("a.txt", "two\n");
    scratch.file("b.txt", "new\n");
    let shown = diff(scratch.path());
    assert!(shown.contains("-one") && shown.contains("+two"), "{shown}");
    assert!(shown.contains("new file (untracked): b.txt"), "{shown}");
}

#[test]
fn copy_takes_the_last_answer_that_says_something() {
    let turn = |answer: &str| contracts::history::Turn {
        goal: "x".into(),
        answer: answer.into(),
        status: "achieved".into(),
        at: 0,
        steps: vec![],
    };
    // A failed or stopped task leaves no answer: the one before it is copied, never nothing.
    assert_eq!(
        last_answer(&[turn(" ok \n"), turn(""), turn("   ")]).as_deref(),
        Some("ok")
    );
    assert_eq!(last_answer(&[turn(""), turn("  ")]), None);
    assert_eq!(last_answer(&[]), None);
}

#[test]
fn usage_counts_tokens_with_thousands_separators_as_the_trace_does() {
    crate::testing::composed(crate::modes::built("default"), false, vec![], |rig| {
        rig.spent.calls = 4;
        rig.spent.input_tokens = 1_234_567;
        rig.spent.cached_tokens = 2_000;
        rig.spent.output_tokens = 1_080;
        let shown = usage(rig);
        assert!(
            shown.contains("4 turns · 1,234,567 in (2,000 cached) · 1,080 out"),
            "{shown}"
        );
        rig.spent.cached_tokens = 0;
        rig.spent.cost = 0.25;
        let shown = usage(rig);
        assert!(shown.contains("· 1,234,567 in · 1,080 out"), "{shown}");
        // The agent's cost is said once, the time on a line of its own.
        assert_eq!(shown.matches("$0.2500").count(), 1, "{shown}");
        assert!(shown.contains("\ntime       0 min\n"), "{shown}");
        0
    });
}

#[test]
fn usage_says_what_the_whole_session_spent_its_earlier_tasks_too() {
    crate::testing::composed(crate::modes::built("default"), false, vec![], |rig| {
        rig.output.spend(contracts::history::Spent {
            input_tokens: 24_100,
            cached_tokens: 21_000,
            output_tokens: 1_200,
            cost: 0.042,
        });
        let shown = usage(rig);
        assert!(
            shown.contains("session    24,100 in (21,000 cached) · 1,200 out · $0.0420"),
            "{shown}"
        );
        0
    });
}

#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn copy_sends_the_last_answer_to_the_client_whose_clipboard_it_is() {
    use protocol::Event;
    let replies = vec![crate::testing::says("forty-two")];
    crate::testing::composed(crate::modes::built("default"), true, replies, |rig| {
        let (ui, frames) = crate::testing::fake_ui();
        rig.ui = Some(ui);
        assert_eq!(copy(rig), Some("No answer to copy yet.\n"));
        rig.task("the answer?");
        assert_eq!(copy(rig), None, "the client says what came of it");
        let copied: Vec<_> = (crate::testing::events(&frames).into_iter())
            .filter_map(|(_, event)| match event {
                Event::Copy(text) => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(copied, ["forty-two"]);
        0
    });
}
