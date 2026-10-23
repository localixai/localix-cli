use std::process::Command;

use super::*;
use crate::testing::Scratch;

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
    crate::testing::composed(crate::mode::Mode::Default, false, vec![], |rig| {
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
        let shown = usage(rig);
        assert!(shown.contains("· 1,234,567 in · 1,080 out"), "{shown}");
        0
    });
}
