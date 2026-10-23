use serde_json::{Value, json};

use super::*;

/// What a call running is shown doing: `Running cargo test`.
fn doing(name: &str, args: &Value) -> String {
    let label = label(name, args);
    format!("{} {}", label.doing, label.subject)
}

/// What a finished call is shown to have done: `Ran cargo test`.
fn did(name: &str, args: &Value) -> String {
    let label = label(name, args);
    format!("{} {}", label.done, label.subject)
}

#[test]
fn commands_files_and_questions_read_as_what_they_do() {
    assert_eq!(
        did("bash", &json!({"command": "cargo  test\n --all"})),
        "Ran cargo test --all"
    );
    assert_eq!(
        did("bash", &json!({"command": "ls", "workdir": "src"})),
        "Ran ls (in src)"
    );
    assert_eq!(
        did("read", &json!({"path": "src/lib.rs"})),
        "Read src/lib.rs"
    );
    assert_eq!(
        did("read", &json!({"path": "src/lib.rs", "offset": 40})),
        "Read src/lib.rs from line 40"
    );
    assert_eq!(
        did(
            "edit",
            &json!({"path": "a.rs", "old_string": "a", "new_string": "b"})
        ),
        "Edited a.rs"
    );
    assert_eq!(
        did(
            "edit",
            &json!({"path": "index.html", "old_string": "", "new_string": "<h1>"})
        ),
        "Created index.html"
    );
    assert_eq!(
        doing("ask", &json!({"questions": [{}, {}]})),
        "Asking 2 questions"
    );
    assert_eq!(did("ask", &json!({"questions": [{}]})), "Asked 1 question");
    assert_eq!(doing("mystery", &json!({})), "Calling mystery");
}

#[test]
fn agents_and_jobs_read_as_their_action() {
    assert_eq!(
        doing(
            "agents",
            &json!({"action": "spawn", "name": "Docs", "task": "read the docs"})
        ),
        "Spawning docs · read the docs"
    );
    assert_eq!(
        did("jobs", &json!({"action": "stop", "job": "j2"})),
        "Stopped job j2"
    );
}

#[test]
fn a_window_call_says_what_it_opens_and_which_tab_it_acts_in() {
    let said = |args: Value| doing("window", &args);
    assert_eq!(said(json!({"action": "list"})), "Listing the window's tabs");
    assert_eq!(
        said(json!({"action": "show", "tab": "t2"})),
        "Showing tab t2"
    );
    assert_eq!(
        said(json!({"action": "close", "tabs": ["w1", "t2"]})),
        "Closing tabs w1, t2"
    );
    assert_eq!(
        said(json!({"action": "open", "kind": "browser", "url": "github.com"})),
        "Opening github.com"
    );
    assert_eq!(
        said(json!({"action": "open", "tabs": [
            {"kind": "terminal", "command": "claude"},
            {"kind": "browser", "url": "example.org"}
        ]})),
        "Opening claude, example.org"
    );
    assert_eq!(
        said(json!({"action": "interact", "tab": "t1", "goal": "Answer yes"})),
        "Working in t1: Answer yes"
    );
    assert_eq!(
        said(json!({"action": "interact", "tab": "t1", "input": "hi", "keys": ["Enter"]})),
        "Typing in t1: hi [Enter]"
    );
    assert_eq!(
        said(json!({"action": "screenshot", "tab": "w1"})),
        "Picturing tab w1"
    );
    assert_eq!(
        said(json!({"action": "resize", "width": 1100, "height": 760})),
        "Sizing the window to 1100×760"
    );
}
