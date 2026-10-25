//! Modes as the person writes them: named sets of rules in `<config>/llx/settings.json`, chosen
//! with `--mode`, replacing a built-in one of the same name, and refused when they cannot be read.

use serde_json::json;

use crate::common::home::{Home, text};
use crate::common::model::{Model, Reply, said};
use crate::suite::exec::exec;

/// A home whose settings hold `modes`.
fn with_modes(modes: serde_json::Value) -> Home {
    let home = Home::new();
    let dir = home.path().join("config/llx");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("settings.json"),
        json!({ "modes": modes }).to_string(),
    )
    .unwrap();
    home
}

fn running(command: &str) -> Model {
    Model::start(vec![
        Reply::call("bash", json!({"command": command})),
        Reply::text("Done.\nRESULT: achieved"),
    ])
}

#[test]
fn a_mode_of_the_persons_runs_what_it_allows_and_the_model_is_told_its_rules() {
    let home = with_modes(
        json!({"review": {"allow": ["bash(echo *)"], "description": "Review with checks."}}),
    );
    let model = running("echo checked");
    let out = exec(&home, &model, &["exec", "--mode", "review", "x"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let context = said(&model, 0);
    assert!(
        context.contains("review") && context.contains("bash(echo *)"),
        "{context}"
    );
    assert!(said(&model, 1).contains("checked"));
}

#[test]
fn the_persons_mode_replaces_a_built_in_one_of_the_same_name() {
    let home = with_modes(json!({"plan": {"allow": ["bash(echo *)"], "deny": ["edit"]}}));
    let model = running("echo checked");
    let out = exec(&home, &model, &["exec", "--mode", "plan", "x"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(said(&model, 1).contains("checked"), "{}", said(&model, 1));
}

#[test]
fn flags_add_rules_to_the_mode() {
    let home = with_modes(json!({"review": {"allow": ["bash(echo *)"]}}));
    let model = running("echo checked");
    let args = ["exec", "--mode", "review", "--deny", "bash(echo *)", "x"];
    let out = exec(&home, &model, &args, "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(
        said(&model, 1).contains("bash(echo *)"),
        "refused by the flag's rule"
    );
}

#[test]
fn an_unknown_mode_is_not_ready_and_the_modes_are_listed() {
    let home = with_modes(json!({"review": {"allow": ["bash"]}}));
    for name in ["nosuch", "autonomous"] {
        let out = exec(
            &home,
            &running("echo hi"),
            &["exec", "--mode", name, "x"],
            "",
        );
        assert_eq!(out.status.code(), Some(3), "{name}");
        let stderr = text(&out.stderr);
        assert!(stderr.contains("default, plan, bypass, review"), "{stderr}");
    }
}

#[test]
fn a_mode_that_cannot_be_read_is_not_ready_and_says_where() {
    let home = with_modes(json!({"broken": {"allow": ["bash(echo *"]}}));
    let out = exec(&home, &running("echo hi"), &["exec", "x"], "");
    assert_eq!(out.status.code(), Some(3));
    let stderr = text(&out.stderr);
    assert!(
        stderr.contains("settings.json") && stderr.contains("broken"),
        "{stderr}"
    );
}

#[test]
fn plan_refuses_a_file_change_and_bypass_allows_it() {
    for (mode, made) in [("plan", false), ("bypass", true)] {
        let home = Home::new();
        let model = Model::start(vec![
            Reply::call(
                "edit",
                json!({"path": "a.md", "old_string": "", "new_string": "a\n"}),
            ),
            Reply::text("Done.\nRESULT: achieved"),
        ]);
        let out = exec(&home, &model, &["exec", "--mode", mode, "x"], "");
        assert_eq!(out.status.code(), Some(0), "{mode}: {}", text(&out.stderr));
        assert_eq!(home.path().join("a.md").exists(), made, "{mode}");
    }
}
