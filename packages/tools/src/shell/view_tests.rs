use contracts::view::{Action, View};

use super::*;
use crate::shell::session::Wait;
use crate::testing::Scratch;

fn never() -> bool {
    false
}

#[test]
fn a_session_reads_as_its_screen_with_an_input_and_keys_and_is_typed_into() {
    let (dir, sessions) = (Scratch::new(), Sessions::default());
    let wait = Wait {
        needle: Some("name?"),
        yield_ms: Some(5000),
    };
    let turn = sessions
        .start(
            "printf 'name? '; read n; echo \"hi $n\"; sleep 30",
            dir.path(),
            Default::default(),
            wait,
            None,
            &never,
        )
        .unwrap();
    let view = TerminalView {
        sessions: &sessions,
        id: turn.id,
        stop: &never,
    };
    let seen = view.observe().unwrap();
    assert!(seen.outline.starts_with("name?"), "{}", seen.outline);
    assert_eq!(seen.location, format!("session {}", turn.id));
    let refs: Vec<&str> = seen.elements.iter().map(|e| e.reference.as_str()).collect();
    assert_eq!(refs[0], "input");
    assert!(
        refs.contains(&"key:Enter") && refs.contains(&"key:Down"),
        "{refs:?}"
    );
    assert!(seen.elements[0].editable);
    let typed = Action::Type {
        reference: "input".into(),
        text: "Ann".into(),
    };
    view.act(&seen.epoch, &typed).unwrap();
    let enter = Action::Click {
        reference: "key:Enter".into(),
    };
    view.act(&seen.epoch, &enter).unwrap();
    let after = view.observe().unwrap();
    assert!(after.outline.contains("hi Ann"), "{}", after.outline);
    assert_ne!(after.epoch, seen.epoch, "a new screen, a new epoch");
    // Nothing else is a control here.
    let other = Action::Click {
        reference: "e1".into(),
    };
    assert!(view.act(&after.epoch, &other).is_err());
    assert!(!view.conventions().is_empty());
}
