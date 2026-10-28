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
fn commands_and_questions_read_as_what_they_do() {
    assert_eq!(
        did("bash", &json!({"command": "cargo  test\n --all"})),
        "Ran cargo test --all"
    );
    assert_eq!(
        did("bash", &json!({"command": "ls", "workdir": "src"})),
        "Ran ls (in src)"
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
