//! `--allow` and `--deny` as a pipeline uses them: with no one at a terminal, what an allow rule
//! names runs (and only what it names), what a deny rule names is refused in every mode, and the
//! model is told which commands it may run.

use serde_json::json;

use crate::common::home::{Home, text};
use crate::common::model::{Model, Reply, said};
use crate::suite::exec::exec;

/// A task that runs `command`, then answers.
fn running(command: &str) -> Model {
    Model::start(vec![
        Reply::call("bash", json!({"command": command})),
        Reply::text("Done.\nRESULT: achieved"),
    ])
}

#[test]
fn plan_mode_runs_an_allowed_command_and_tells_the_model_it_may() {
    let (home, model) = (Home::new(), running("echo checked"));
    let args = [
        "exec",
        "--mode",
        "plan",
        "--allow",
        "bash(echo *)",
        "check it",
    ];
    let out = exec(&home, &model, &args, "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let first = said(&model, 0);
    assert!(
        first.contains("bash(echo *)"),
        "the context names the rule: {first}"
    );
    let told = said(&model, 1);
    assert!(
        told.contains("checked") && !told.contains("not allowed"),
        "{told}"
    );
}

#[test]
fn an_allowed_command_chained_to_another_is_refused() {
    let home = Home::new();
    for command in [
        "echo hi && touch made.txt",
        "echo hi; touch made.txt",
        "echo $(touch made.txt)",
        "echo hi > made.txt",
    ] {
        let model = running(command);
        let args = ["exec", "--mode", "plan", "--allow", "bash(echo *)", "x"];
        let out = exec(&home, &model, &args, "");
        assert_eq!(
            out.status.code(),
            Some(0),
            "{command}: {}",
            text(&out.stderr)
        );
        assert!(!home.path().join("made.txt").exists(), "{command} ran");
        assert!(said(&model, 1).contains("not allowed"), "{command}");
    }
}

#[test]
fn a_deny_rule_refuses_even_with_yes_and_names_itself() {
    let (home, model) = (Home::new(), running("touch made.txt"));
    let args = ["exec", "-y", "--deny", "bash(touch *)", "x"];
    let out = exec(&home, &model, &args, "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(!home.path().join("made.txt").exists());
    let told = said(&model, 1);
    assert!(told.contains("bash(touch *)"), "{told}");
}

#[test]
fn an_edit_rule_allows_the_paths_it_names_and_no_others() {
    let home = Home::new();
    let model = Model::start(vec![
        Reply::call(
            "edit",
            json!({"path": "docs/a.md", "old_string": "", "new_string": "a\n"}),
        ),
        Reply::call(
            "edit",
            json!({"path": "src/b.rs", "old_string": "", "new_string": "b\n"}),
        ),
        Reply::text("Done.\nRESULT: achieved"),
    ]);
    let out = exec(
        &home,
        &model,
        &["exec", "--allow", "edit(docs/**)", "x"],
        "",
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(home.path().join("docs/a.md").exists());
    assert!(!home.path().join("src/b.rs").exists());
}

#[test]
fn plan_mode_stays_read_only_whatever_is_allowed() {
    let home = Home::new();
    let model = Model::start(vec![
        Reply::call(
            "edit",
            json!({"path": "a.md", "old_string": "", "new_string": "a\n"}),
        ),
        Reply::text("Done.\nRESULT: achieved"),
    ]);
    let out = exec(
        &home,
        &model,
        &["exec", "--mode", "plan", "--allow", "edit", "x"],
        "",
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(!home.path().join("a.md").exists());
}

#[test]
fn a_rule_that_cannot_be_read_is_not_ready() {
    let home = Home::new();
    for rule in ["bash(echo *", "nosuchtool", "edit(//etc/**)"] {
        let model = running("echo hi");
        let out = exec(&home, &model, &["exec", "--allow", rule, "x"], "");
        assert_eq!(out.status.code(), Some(3), "{rule}");
        assert!(
            text(&out.stderr).contains(rule),
            "{rule}: {}",
            text(&out.stderr)
        );
    }
}
