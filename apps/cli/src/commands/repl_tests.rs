use super::*;
use crate::testing::{answers, composed, users};

#[test]
fn slash_commands_are_not_tasks() {
    use protocol::{LESS_REASONING, MORE_REASONING};
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
fn mode_commands_switch_and_list() {
    assert_eq!(entry("/mode"), Some(Entry::Modes));
    assert_eq!(entry("/permissions"), Some(Entry::Modes));
    assert_eq!(entry("/mode plan"), Some(Entry::Switch("plan".into())));
    assert_eq!(
        entry("/permissions  review "),
        Some(Entry::Switch("review".into()))
    );
    assert_eq!(entry("\u{1b}[Z"), Some(Entry::CycleMode));
    assert_eq!(entry("\u{1b}[Z\n"), Some(Entry::CycleMode));
    assert_eq!(entry("/plan"), Some(Entry::Unknown("plan".into())));
    assert_eq!(entry("/exit now"), Some(Entry::Unknown("exit now".into())));
}

#[test]
fn switching_to_a_mode_that_is_not_one_changes_nothing() {
    composed(crate::modes::built("default"), true, answers(0), |rig| {
        assert_eq!(command(rig, Entry::Switch("nope".into())), None);
        assert_eq!(rig.mode.get().name(), "default");
        assert_eq!(command(rig, Entry::Switch("plan".into())), None);
        assert_eq!(rig.mode.get().name(), "plan");
        0
    });
}

#[test]
fn shift_tab_cycles_every_mode_and_tells_the_next_task() {
    composed(crate::modes::built("default"), true, answers(6), |rig| {
        for expected in [
            crate::modes::built("plan"),
            crate::modes::built("bypass"),
            crate::modes::built("default"),
        ]
        .into_iter()
        .cycle()
        .take(6)
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
