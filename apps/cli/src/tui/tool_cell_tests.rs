use contracts::tool::{ToolError, ToolOutput};
use serde_json::{Value, json};

use super::*;
use crate::tui::style;

fn done(report: &str) -> Result<ToolOutput, ToolError> {
    Ok(ToolOutput {
        report: report.into(),
        state: None,
        image: None,
    })
}

fn shown(name: &str, args: Value, state: State, width: usize) -> String {
    let lines = lines(name, &args, state, width);
    lines
        .iter()
        .map(style::plain)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_command_shows_the_ends_of_what_it_printed() {
    let printed: Vec<String> = (1..=12).map(|n| format!("line {n}")).collect();
    let report = format!("exit 0 in 0.4s\n### stdout\n{}", printed.join("\n"));
    insta::assert_snapshot!(shown("bash", json!({"command": "seq 12 | sed 's/^/line /'"}), State::Done(&done(&report)), 80), @r"
    • Ran seq 12 | sed 's/^/line /'
      └ line 1
        line 2
        … +7 lines
        line 10
        line 11
        line 12
    ");
}

#[test]
fn a_long_command_wraps_under_a_rule_and_long_output_is_cut() {
    let report = format!("exit 0 in 1.0s\n### stdout\n{}", "x".repeat(100));
    insta::assert_snapshot!(shown("bash", json!({"command": "find . -maxdepth 3 -not -path '*/node_modules/*' -not -path '*/.git/*' -name '*.rs'"}), State::Done(&done(&report)), 48), @r"
    • Ran find . -maxdepth 3 -not -path
      │ '*/node_modules/*' -not -path '*/.git/*'
      │ -name '*.rs'
      └ xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx…
    ");
}

#[test]
fn a_failing_command_ends_with_how_it_exited_in_red() {
    let outcome = done("exit 101 in 2.1s\n### stderr\nerror[E0425]: cannot find value `x`");
    let lines = lines(
        "bash",
        &json!({"command": "cargo build"}),
        State::Done(&outcome),
        80,
    );
    insta::assert_snapshot!(lines.iter().map(style::plain).collect::<Vec<_>>().join("\n"), @r"
    • Ran cargo build
      └ error[E0425]: cannot find value `x`
        exit 101 in 2.1s
    ");
    assert_eq!(lines[0].spans[0].style, style::failed());
    assert_eq!(lines[2].spans[1].style, style::failed());
}

#[test]
fn a_command_with_no_output_says_so() {
    insta::assert_snapshot!(shown("bash", json!({"command": "touch a"}), State::Done(&done("exit 0 in 0.0s, no output")), 80), @r"
    • Ran touch a
      └ (no output)
    ");
}

#[test]
fn a_running_call_has_a_dim_bullet_and_a_present_verb() {
    let lines = lines(
        "bash",
        &json!({"command": "cargo test"}),
        State::Running,
        80,
    );
    assert_eq!(style::plain(&lines[0]), "• Running cargo test");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].style, style::dim());
}

#[test]
fn files_say_what_changed() {
    let edited = done("Edited src/a.rs: replaced 1 occurrence (+3 -1 lines).\n12 fn a() {}");
    let created = done("Created site/index.html (48 lines).");
    insta::assert_snapshot!([
        shown("read", json!({"path": "src/lib.rs"}), State::Done(&done("1 use x;")), 80),
        shown("read", json!({"path": "src/lib.rs", "offset": 40}), State::Done(&done("40 x")), 80),
        shown("edit", json!({"path": "src/a.rs", "old_string": "a", "new_string": "b"}), State::Done(&edited), 80),
        shown("edit", json!({"path": "site/index.html", "old_string": "", "new_string": "<h1>"}), State::Done(&created), 80),
    ].join("\n"), @r"
    • Read src/lib.rs
    • Read src/lib.rs from line 40
    • Edited src/a.rs
      └ (+3 -1 lines)
    • Created site/index.html
      └ (48 lines)
    ");
}

#[test]
fn a_failed_call_shows_why() {
    let outcome = Err(ToolError::Failed(
        "old_string was not found in src/a.rs".into(),
    ));
    insta::assert_snapshot!(shown("edit", json!({"path": "src/a.rs", "old_string": "a"}), State::Done(&outcome), 80), @r"
    • Edited src/a.rs
      └ old_string was not found in src/a.rs
    ");
}

#[test]
fn sessions_browser_and_questions() {
    insta::assert_snapshot!([
        shown("bash", json!({"command": "python3 -q", "tty": true}), State::Running, 80),
        shown("bash", json!({"session": 1, "input": "print(1)\n"}), State::Running, 80),
        shown("bash", json!({"session": 1}), State::Running, 80),
        shown("bash", json!({"session": 1, "close": true}), State::Running, 80),
        shown("browser", json!({"url": "http://127.0.0.1:4173/", "goal": "Click Buy; done when the cart shows 1"}), State::Done(&done("Done: the cart shows 1.\nmore")), 80),
        shown("ask", json!({"questions": [{}, {}]}), State::Running, 80),
    ].join("\n"), @r"
    • Starting python3 -q
    • Typing into session 1: print(1)
    • Reading session 1
    • Closing session 1
    • Browsed http://127.0.0.1:4173/ · Click Buy; done when the cart shows 1
      └ Done: the cart shows 1.
    • Asking 2 questions
    ");
}

#[test]
fn control_characters_cannot_reach_the_terminal() {
    let text = shown(
        "bash",
        json!({"command": "echo \u{1b}[2J"}),
        State::Done(&done("exit 0 in 0.0s\n### stdout\n\u{1b}[2Jhi")),
        80,
    );
    assert!(!text.contains('\u{1b}'), "{text}");
}

#[test]
fn a_question_shows_what_the_person_answered() {
    let outcome = done("The person answered:\n- colour: Blue");
    insta::assert_snapshot!(shown("ask", json!({"questions": [{}]}), State::Done(&outcome), 80), @r"
    • Asked 1 question
      └ - colour: Blue
    ");
}

#[test]
fn a_browser_call_says_what_it_takes() {
    insta::assert_snapshot!([
        shown("browser", json!({"screenshot": true, "selector": "#board"}), State::Running, 80),
        shown("browser", json!({"script": "1 + 1"}), State::Running, 80),
        shown("browser", json!({"close": true}), State::Running, 80),
        shown("browser", json!({}), State::Running, 80),
    ].join("\n"), @r"
    • Browsing a screenshot of #board
    • Browsing a script
    • Closing the browser
    • Reading the page
    ");
}
