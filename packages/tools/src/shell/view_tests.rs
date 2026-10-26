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
    let view = TerminalView::new(&sessions, turn.id, &never).unwrap();
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

#[test]
fn what_the_helper_made_appear_is_still_new_output_for_the_next_call() {
    let (dir, sessions) = (Scratch::new(), Sessions::default());
    let wait = |needle| Wait {
        needle,
        yield_ms: Some(5000),
    };
    let turn = sessions
        .start(
            "printf 'name? '; read n; echo \"hi $n\"; sleep 30",
            dir.path(),
            Default::default(),
            wait(Some("name?")),
            None,
            &never,
        )
        .unwrap();
    let view = TerminalView::new(&sessions, turn.id, &never).unwrap();
    let seen = view.observe().unwrap();
    let typed = Action::Type {
        reference: "input".into(),
        text: "Ann".into(),
    };
    view.act(&seen.epoch, &typed).unwrap();
    let enter = Action::PressEnter {
        reference: Some("input".into()),
    };
    view.act(&seen.epoch, &enter).unwrap();
    // The agent reads the tab after the goal: the answer the helper's Enter brought is there.
    let read = sessions
        .send(turn.id, b"", &[], wait(None), None, &never)
        .unwrap();
    let output = read.output.unwrap_or_default();
    assert!(output.contains("hi Ann"), "{output}");
}

#[test]
fn a_goal_reads_what_came_since_it_began_apart_from_what_was_there() {
    let (dir, sessions) = (Scratch::new(), Sessions::default());
    let wait = Wait {
        needle: Some("ready"),
        yield_ms: Some(5000),
    };
    let turn = sessions
        .start(
            "echo earlier; echo ready; while read line; do echo \"got $line\"; done",
            dir.path(),
            Default::default(),
            wait,
            None,
            &never,
        )
        .unwrap();
    let typed = |view: &TerminalView| {
        let seen = view.observe().unwrap();
        let text = Action::Type {
            reference: "input".into(),
            text: "x".into(),
        };
        view.act(&seen.epoch, &text).unwrap();
        let enter = Action::PressEnter { reference: None };
        view.act(&seen.epoch, &enter).unwrap();
        view.observe().unwrap().outline
    };
    let parted = |outline: &str| {
        let (before, since) = outline.split_once(BEGAN).unwrap_or(("", outline));
        (before.to_string(), since.to_string())
    };
    let first = TerminalView::new(&sessions, turn.id, &never).unwrap();
    let (before, since) = parted(&typed(&first));
    assert!(
        before.contains("earlier") && !since.contains("earlier"),
        "{before}|{since}"
    );
    assert!(since.contains("got x"), "{since}");
    // The same line again in the next goal: the answer the first one had is above the line.
    let second = TerminalView::new(&sessions, turn.id, &never).unwrap();
    let (before, since) = parted(&typed(&second));
    assert_eq!(before.matches("got x").count(), 1, "{before}");
    assert_eq!(since.matches("got x").count(), 1, "{since}");
}

#[test]
fn a_fresh_screen_has_no_line_between_before_and_since() {
    let (dir, sessions) = (Scratch::new(), Sessions::default());
    let wait = Wait {
        needle: Some("$ "),
        yield_ms: Some(5000),
    };
    let turn = sessions
        .start(
            "printf '$ '; sleep 30",
            dir.path(),
            Default::default(),
            wait,
            None,
            &never,
        )
        .unwrap();
    let view = TerminalView::new(&sessions, turn.id, &never).unwrap();
    assert!(!view.observe().unwrap().outline.contains(BEGAN));
}
