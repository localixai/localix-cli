use std::cell::RefCell;
use std::io::Cursor;

use super::*;
use crate::testing::{answers, composed, users};

fn lines(input: &str) -> (Vec<String>, String) {
    raw(input.as_bytes())
}

fn raw(input: &[u8]) -> (Vec<String>, String) {
    let mut input = Cursor::new(input.to_vec());
    let prompts = RefCell::new(String::new());
    let shown = |text: &str| prompts.borrow_mut().push_str(text);
    let mut read_all = vec![];
    while let Some(text) = read(&mut input, &shown, "\n› ") {
        read_all.push(text);
    }
    (read_all, prompts.into_inner())
}

#[test]
fn each_line_is_an_entry_and_the_end_of_input_quits() {
    let (entries, prompts) = lines("fix the test\n\nrun it\r\n");
    assert_eq!(entries, ["fix the test", "", "run it"]);
    assert_eq!(prompts.matches("› ").count(), 4, "one prompt per read");
}

#[test]
fn a_backslash_goes_on_on_the_next_line() {
    let (entries, _) = lines("first part \\\nsecond part\nnext\n");
    assert_eq!(entries, ["first part \nsecond part", "next"]);
}

#[test]
fn a_last_line_without_a_newline_still_counts() {
    assert_eq!(lines("only this").0, ["only this"]);
    assert_eq!(lines("cut \\").0, ["cut \n"]);
}

#[test]
fn slash_commands_are_not_tasks() {
    assert_eq!(entry("   "), None);
    assert_eq!(entry(" /exit "), Some(Entry::Exit));
    assert_eq!(entry("/quit"), Some(Entry::Exit));
    assert_eq!(entry("/help"), Some(Entry::Help));
    assert_eq!(entry("/new"), Some(Entry::New));
    assert_eq!(entry("/clear"), Some(Entry::New));
    assert_eq!(entry("/status"), Some(Entry::Status));
    assert_eq!(entry(" /login "), Some(Entry::Login));
    assert_eq!(
        entry("/login openai"),
        Some(Entry::Unknown("login openai".into()))
    );
    assert_eq!(entry("/undo"), Some(Entry::Unknown("undo".into())));
    assert_eq!(
        entry("add a test for /api/users"),
        Some(Entry::Task("add a test for /api/users".into()))
    );
}

#[test]
fn a_line_that_is_not_utf8_is_read_not_taken_for_the_end() {
    let (entries, _) = raw(b"caf\xe9 latin-1\nnext\n");
    assert_eq!(entries, ["caf\u{fffd} latin-1", "next"]);
}

#[test]
fn a_read_error_ends_the_line_and_says_why() {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("the terminal is gone"))
        }
    }
    let mut input = std::io::BufReader::new(Broken);
    let said = RefCell::new(String::new());
    let shown = |text: &str| said.borrow_mut().push_str(text);
    assert_eq!(read(&mut input, &shown, "\n› "), None);
    assert!(
        said.borrow()
            .contains("could not be read: the terminal is gone")
    );
}

#[test]
fn the_prompt_shows_the_mode_unless_it_is_the_default() {
    assert_eq!(prompt(Mode::Default), "\n› ");
    assert_eq!(prompt(Mode::Plan), "\nplan › ");
    let mut input = Cursor::new(b"go\n".to_vec());
    let said = RefCell::new(String::new());
    let shown = |text: &str| said.borrow_mut().push_str(text);
    assert_eq!(
        read(&mut input, &shown, &prompt(Mode::Bypass)),
        Some("go".into())
    );
    assert_eq!(said.into_inner(), "\nbypass › ");
}

#[test]
fn mode_commands_switch_list_and_plan() {
    assert_eq!(entry("/mode"), Some(Entry::Modes));
    assert_eq!(entry("/permissions"), Some(Entry::Modes));
    assert_eq!(entry("/mode plan"), Some(Entry::Switch(Mode::Plan)));
    assert_eq!(
        entry("/permissions  Bypass "),
        Some(Entry::Switch(Mode::Bypass))
    );
    assert_eq!(entry("/mode nope"), Some(Entry::UnknownMode("nope".into())));
    assert_eq!(entry("/plan"), Some(Entry::Plan(None)));
    assert_eq!(
        entry("/plan fix x"),
        Some(Entry::Plan(Some("fix x".into())))
    );
    assert_eq!(entry("\u{1b}[Z"), Some(Entry::TogglePlan));
    assert_eq!(entry("\u{1b}[Z\n"), Some(Entry::TogglePlan));
    assert_eq!(entry("/exit now"), Some(Entry::Unknown("exit now".into())));
}

#[test]
fn the_handoff_is_yes_no_or_the_next_entry() {
    assert_eq!(handoff(Some("y\n")), Handoff::Implement);
    assert_eq!(handoff(Some(" Yes ")), Handoff::Implement);
    assert_eq!(handoff(Some("n")), Handoff::Stay);
    assert_eq!(handoff(Some("")), Handoff::Stay);
    assert_eq!(handoff(None), Handoff::Stay);
    assert_eq!(
        handoff(Some("add tests to step 3\n")),
        Handoff::Next("add tests to step 3".into())
    );
    assert_eq!(
        handoff(Some("/mode bypass")),
        Handoff::Next("/mode bypass".into())
    );
}

/// The plan handoff as a session runs it: plan mode entered from bypass, a plan task that ends
/// well, then the answer to "Implement this plan?" read from the prompt line's input.
fn handed_off(answer: &'static str, replies: usize, check: impl FnOnce(&Rig, Next)) {
    composed(Mode::Bypass, true, answers(replies), |rig| {
        rig.switch(Mode::Plan);
        let mut input = Cursor::new(answer.as_bytes().to_vec());
        let next = attempt(rig, "plan the change", &mut input, &|_| {});
        check(rig, next);
        0
    });
}

#[test]
fn yes_carries_out_the_plan_in_the_mode_it_came_from() {
    handed_off("y\n", 2, |rig, next| {
        assert!(matches!(next, Next::Prompt));
        assert_eq!(rig.mode.get(), Mode::Bypass);
        let users = users(rig);
        assert_eq!(users.len(), 4, "{users:?}");
        assert!(users[0].starts_with("<mode_change>\nThe mode is now plan: "));
        assert_eq!(users[1], "Goal: plan the change");
        assert!(
            users[2].starts_with("<mode_change>\nThe mode is now bypass: "),
            "the mode changed back before the plan was carried out: {users:?}"
        );
        assert_eq!(users[3], format!("Goal: {IMPLEMENT}"));
    });
}

#[test]
fn no_stays_in_plan_and_anything_else_is_the_next_entry() {
    handed_off("n\n", 1, |rig, next| {
        assert!(matches!(next, Next::Prompt));
        assert_eq!(rig.mode.get(), Mode::Plan);
        assert_eq!(users(rig)[1..], ["Goal: plan the change"]);
    });
    handed_off("add tests to step 3\n", 1, |rig, next| {
        assert!(matches!(next, Next::Entry(text) if text == "add tests to step 3"));
        assert_eq!(rig.mode.get(), Mode::Plan);
        assert_eq!(users(rig).len(), 2, "nothing ran yet");
    });
}

#[test]
fn a_plan_that_failed_or_a_task_outside_plan_is_not_handed_off() {
    // The script has no answer: the task fails, and nothing is asked or read.
    handed_off("y\n", 0, |rig, next| {
        assert!(matches!(next, Next::Prompt));
        assert_eq!(rig.mode.get(), Mode::Plan);
    });
    composed(Mode::Bypass, true, answers(1), |rig| {
        let mut input = Cursor::new(b"y\n".to_vec());
        let next = attempt(rig, "do it", &mut input, &|_| {});
        assert!(matches!(next, Next::Prompt));
        assert_eq!(input.position(), 0, "nothing was asked");
        0
    });
}
