use serde_json::json;

use super::*;

#[test]
fn each_action_is_read_with_what_it_needs() {
    assert_eq!(
        action(&json!({"action": "spawn", "task": " look ", "name": "API Scout"})),
        Ok(Action::Spawn {
            task: "look".into(),
            name: Some("api-scout".into())
        })
    );
    assert_eq!(
        action(&json!({"action": "send", "agent": "scout", "message": "go on"})),
        Ok(Action::Send {
            agent: "scout".into(),
            message: "go on".into()
        })
    );
    assert_eq!(action(&json!({"action": "list"})), Ok(Action::List));
    assert_eq!(
        action(&json!({"action": "close", "agent": "scout"})),
        Ok(Action::Close {
            agent: "scout".into()
        })
    );
}

#[test]
fn a_call_without_its_action_is_read_from_what_it_gives() {
    assert!(matches!(
        action(&json!({"task": "look"})),
        Ok(Action::Spawn { .. })
    ));
    assert!(matches!(
        action(&json!({"agent": "scout", "message": "more"})),
        Ok(Action::Send { .. })
    ));
    assert!(action(&json!({})).unwrap_err().contains("action"));
}

#[test]
fn what_is_missing_is_said() {
    let missing = |args| action(&args).unwrap_err();
    assert!(missing(json!({"action": "spawn"})).contains("`task`"));
    assert!(missing(json!({"action": "send", "agent": "a"})).contains("`message`"));
    assert!(missing(json!({"action": "close"})).contains("`agent`"));
    assert!(missing(json!({"action": "fly"})).contains("no action `fly`"));
}

#[test]
fn a_name_is_tidied_or_dropped() {
    assert_eq!(tidy("  Test Runner #2 "), Some("test-runner-2".into()));
    assert_eq!(tidy("!!!"), None);
    assert_eq!(tidy(&"x".repeat(40)).unwrap().len(), 24);
}
