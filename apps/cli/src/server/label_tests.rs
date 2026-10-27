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

#[test]
fn an_interact_reads_as_what_it_does_in_the_tab() {
    let said = |args: Value| doing("window", &args);
    let done = |args: Value| did("window", &args);
    // Code run in the page is not a read.
    assert_eq!(
        done(json!({"action": "interact", "tab": "w1", "script": "game.state()"})),
        "Ran a script in w1"
    );
    assert_eq!(
        said(json!({"action": "interact", "tab": "w1", "script": "1"})),
        "Running a script in w1"
    );
    // Keys are pressed, not typed.
    assert_eq!(
        done(json!({"action": "interact", "tab": "w1", "keys": ["ArrowUp"]})),
        "Pressed ArrowUp in w1"
    );
    assert_eq!(
        said(json!({"action": "interact", "tab": "w1", "keys": ["ArrowUp", "ArrowLeft"]})),
        "Pressing ArrowUp ArrowLeft in w1"
    );
    let many: Vec<&str> = vec!["ArrowDown"; 40];
    let long = done(json!({"action": "interact", "tab": "w1", "keys": many}));
    assert!(
        long.starts_with("Pressed ArrowDown ArrowDown") && long.contains("40 keys"),
        "{long}"
    );
    assert!(long.len() < 100, "{long}");
    // Keys and a script: the keys are what the person sees happen.
    assert_eq!(
        done(json!({"action": "interact", "tab": "w1", "keys": ["Enter"], "script": "x"})),
        "Pressed Enter in w1, then ran a script"
    );
    assert_eq!(
        done(json!({"action": "interact", "tab": "w1", "url": "example.org/a"})),
        "Went to example.org/a in w1"
    );
    assert_eq!(
        done(json!({"action": "interact", "tab": "t1", "wait_for": "ready"})),
        "Waited in t1 for \"ready\""
    );
    assert_eq!(
        done(json!({"action": "interact", "tab": "w1", "size": "mobile"})),
        "Reframed w1 as mobile"
    );
    assert_eq!(
        done(json!({"action": "interact", "tab": "w1", "read": true})),
        "Read w1"
    );
    assert_eq!(done(json!({"action": "interact", "tab": "w1"})), "Read w1");
    assert_eq!(
        done(json!({"action": "interact", "tab": "w1", "screenshot": true})),
        "Read w1"
    );
    // A tab left out is the one the window acts in.
    assert_eq!(
        done(json!({"action": "interact", "script": "1"})),
        "Ran a script in the current tab"
    );
    assert_eq!(
        done(json!({"action": "open", "kind": "browser"})),
        "Opened a browser tab"
    );
    assert_eq!(
        done(
            json!({"action": "open", "tabs": [{"kind": "terminal"}, {"kind": "browser", "url": "a.org"}]})
        ),
        "Opened a terminal tab, a.org"
    );
}
