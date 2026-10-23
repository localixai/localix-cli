use super::*;
use crate::testing::{answers, composed, users};

#[test]
fn slash_commands_are_not_tasks() {
    use crate::commands::entry::{LESS_REASONING, MORE_REASONING};
    let unknown = |text: &str| Some(Entry::Unknown(text.into()));
    for (typed, read) in [
        ("   ", None),
        (" /exit ", Some(Entry::Exit)),
        ("/quit", Some(Entry::Exit)),
        ("/help", Some(Entry::Help)),
        ("/new", Some(Entry::New)),
        ("/clear", Some(Entry::New)),
        ("/status", Some(Entry::Status)),
        (" /login ", Some(Entry::Login)),
        ("/login openai", unknown("login openai")),
        // Another spelling of /rewind.
        ("/undo", Some(Entry::Rewind)),
        ("/frobnicate", unknown("frobnicate")),
        (
            "add a test for /api/users",
            Some(Entry::Task("add a test for /api/users".into())),
        ),
        // Shift+Up and Shift+Down ask for more or less reasoning.
        (MORE_REASONING, Some(Entry::Reasoning(true))),
        (LESS_REASONING, Some(Entry::Reasoning(false))),
    ] {
        assert_eq!(entry(typed), read, "{typed:?}");
    }
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
    assert_eq!(entry("\u{1b}[Z"), Some(Entry::CycleMode));
    assert_eq!(entry("\u{1b}[Z\n"), Some(Entry::CycleMode));
    assert_eq!(entry("/exit now"), Some(Entry::Unknown("exit now".into())));
}

#[test]
fn shift_tab_cycles_every_mode_and_tells_the_next_task() {
    composed(Mode::Default, true, answers(8), |rig| {
        for expected in [Mode::Plan, Mode::Bypass, Mode::Autonomous, Mode::Default]
            .into_iter()
            .cycle()
            .take(8)
        {
            assert_eq!(command(rig, entry("\u{1b}[Z").unwrap()), None);
            assert_eq!(rig.mode.get(), expected);
            assert_eq!(rig.task("continue"), exit::OK);
            let messages = users(rig);
            assert!(
                messages[messages.len() - 2].starts_with(&format!(
                    "<mode_change>\nThe mode is now {}: ",
                    expected.name()
                )),
                "the next task must hear the cycled mode: {messages:?}"
            );
            assert_eq!(messages.last().unwrap(), "Goal: continue");
        }
        0
    });
}

#[test]
fn shift_tab_leaves_plan_for_bypass_not_the_previous_mode() {
    composed(Mode::Autonomous, true, answers(0), |rig| {
        command(rig, Entry::Plan(None));
        assert_eq!(rig.before_plan, Mode::Autonomous);
        command(rig, entry("\u{1b}[Z").unwrap());
        assert_eq!(rig.mode.get(), Mode::Bypass);
        0
    });
}

/// An interface whose person picks `picked` in every list; the titles of the lists shown.
fn picking(picked: Option<usize>) -> (Ui, std::thread::JoinHandle<Vec<String>>) {
    let (ui, events) = crate::tui::test_support::fake_ui();
    let person = std::thread::spawn(move || {
        let mut titles = vec![];
        for event in events {
            if let Event::Choose { title, reply, .. } = event {
                titles.push(title);
                let _ = reply.send(picked);
            }
        }
        titles
    });
    (ui, person)
}

/// The plan handoff as a session runs it: plan mode entered from bypass, a plan task that ends
/// well, then "Implement this plan?" answered with `picked`; the lists the person was shown.
fn handed_off(
    picked: Option<usize>,
    replies: usize,
    check: impl FnOnce(&Rig, bool),
) -> Vec<String> {
    let (ui, person) = picking(picked);
    composed(Mode::Bypass, true, answers(replies), |rig| {
        rig.switch(Mode::Plan);
        let went_on = attempt(rig, &ui, "plan the change");
        check(rig, went_on);
        0
    });
    drop(ui);
    person.join().unwrap()
}

#[test]
fn yes_carries_out_the_plan_in_the_mode_it_came_from() {
    let asked = handed_off(Some(0), 2, |rig, went_on| {
        assert!(went_on);
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
    assert_eq!(asked, ["Implement this plan?"]);
}

#[test]
fn no_or_a_cancel_stays_in_plan() {
    for picked in [Some(1), None] {
        handed_off(picked, 1, |rig, went_on| {
            assert!(went_on);
            assert_eq!(rig.mode.get(), Mode::Plan);
            assert_eq!(users(rig)[1..], ["Goal: plan the change"]);
        });
    }
}

#[test]
fn a_plan_that_failed_or_a_task_outside_plan_is_not_handed_off() {
    // The script has no answer: the task fails, and nothing is asked.
    let asked = handed_off(Some(0), 0, |rig, _| {
        assert_eq!(rig.mode.get(), Mode::Plan);
    });
    assert!(asked.is_empty(), "{asked:?}");
    let (ui, person) = picking(Some(0));
    composed(Mode::Bypass, true, answers(1), |rig| {
        assert!(attempt(rig, &ui, "do it"));
        0
    });
    drop(ui);
    assert!(person.join().unwrap().is_empty(), "nothing was asked");
}
