use serde_json::json;

use super::*;

#[test]
fn each_action_is_read_with_what_it_needs() {
    let monitor = action(&json!({"action": "monitor", "command": "tail -f x", "name": "Logs"}));
    assert_eq!(
        monitor,
        Ok(Action::Monitor {
            command: "tail -f x".into(),
            name: Some("logs".into()),
            timeout: Duration::from_millis(300_000),
        })
    );
    let schedule = action(&json!({"action": "schedule", "prompt": "check", "in": "45m"}));
    assert!(matches!(
        schedule,
        Ok(Action::Schedule {
            when: When::In(2700),
            recurring: false,
            ..
        })
    ));
    let cron = action(&json!({
        "action": "schedule", "prompt": "check", "cron": "*/5 * * * *", "recurring": true
    }));
    assert!(matches!(
        cron,
        Ok(Action::Schedule {
            when: When::Cron(_),
            recurring: true,
            ..
        })
    ));
    assert_eq!(action(&json!({"action": "list"})), Ok(Action::List));
    assert_eq!(
        action(&json!({"action": "stop", "job": "logs"})),
        Ok(Action::Stop { job: "logs".into() })
    );
}

#[test]
fn what_is_wrong_is_said() {
    let wrong = |args| action(&args).unwrap_err();
    assert!(wrong(json!({"action": "monitor"})).contains("`command`"));
    assert!(wrong(json!({"action": "schedule", "prompt": "p"})).contains("`cron` or `in`"));
    assert!(
        wrong(json!({"action": "schedule", "prompt": "p", "cron": "* *"})).contains("5 fields")
    );
    let once = json!({"action": "schedule", "prompt": "p", "in": "5m", "recurring": true});
    assert!(wrong(once).contains("needs `cron`"));
    assert!(wrong(json!({"action": "fly"})).contains("no action `fly`"));
}

#[test]
fn a_watch_is_bounded() {
    let long = action(&json!({"action": "monitor", "command": "x", "timeout_ms": 99_999_999}));
    assert!(matches!(long, Ok(Action::Monitor { timeout, .. }) if timeout.as_secs() == 1800));
}
