use std::time::Duration;

use super::*;

fn turn(until: Until, output: Option<&str>) -> Turn {
    Turn {
        id: 1,
        command: "python3 -q".into(),
        until,
        waited: Duration::from_millis(300),
        age: Duration::from_millis(4_210),
        output: output.map(str::to_string),
        stalled: None,
    }
}

#[test]
fn a_running_session_says_how_long_it_waited() {
    assert_eq!(
        report(&turn(Until::Settled, Some(">>>")), None, None, ""),
        "session 1 running (`python3 -q`), waited 0.3s\n### output\n>>>"
    );
    assert_eq!(
        report(&turn(Until::Deadline, None), None, None, ""),
        "session 1 running (`python3 -q`), waited 0.3s, no new output"
    );
}

#[test]
fn a_wait_for_text_says_whether_it_appeared() {
    assert_eq!(
        report(
            &turn(Until::Matched, Some("=#")),
            None,
            Some("=#"),
            "select 1;\n"
        ),
        "session 1 running (`python3 -q`): \"=#\" appeared after 0.3s\n### output\n=#"
    );
    assert_eq!(
        report(
            &turn(Until::Matched, Some("42")),
            None,
            Some("42"),
            "print(42)\n"
        ),
        "session 1 running (`python3 -q`): \"42\" appeared after 0.3s; the text was also in the \
         input, so this may be its echo\n### output\n42"
    );
    assert_eq!(
        report(&turn(Until::Deadline, None), None, Some("added"), ""),
        "session 1 running (`python3 -q`): \"added\" did not appear within 0.3s; it may still be \
         working: read again with session 1, or close it, no new output"
    );
}

#[test]
fn an_ended_session_says_how_it_ended() {
    assert_eq!(
        report(
            &turn(Until::Exited(Ending::Exit(0)), Some("bye")),
            None,
            None,
            ""
        ),
        "exit 0 after 4.2s; session 1 ended (`python3 -q`)\n### output\nbye"
    );
    assert_eq!(
        report(
            &turn(Until::Exited(Ending::Signal(2)), None),
            None,
            None,
            ""
        ),
        "killed by signal 2 after 4.2s; session 1 ended (`python3 -q`), no new output"
    );
    // A command that ends while it starts never was a session.
    assert_eq!(
        report(
            &turn(Until::Exited(Ending::Exit(0)), Some("True True")),
            Some("in a terminal"),
            None,
            ""
        ),
        "exit 0 in 4.2s (in a terminal)\n### output\nTrue True"
    );
    assert_eq!(
        report(&turn(Until::Closed { killed: true }, None), None, None, ""),
        "session 1 closed after 4.2s (`python3 -q`): it and everything it started were killed, \
         no new output"
    );
    assert!(
        report(&turn(Until::Closed { killed: false }, None), None, None, "")
            .contains(": it could not be killed, and may still be running")
    );
    assert_eq!(
        report(&turn(Until::Stopped, None), None, None, ""),
        "stopped after 0.3s: session 1 was killed, with everything it started, no new output"
    );
}

#[test]
fn a_stalled_write_is_noted() {
    let stalled = Turn {
        stalled: Some((512, 4000)),
        ..turn(Until::Settled, Some("x"))
    };
    assert_eq!(
        report(&stalled, None, None, ""),
        "session 1 running (`python3 -q`), waited 0.3s\n### output\nx\n\
         (the program is not reading its input: 512 of 4000 bytes sent)"
    );
}

#[test]
fn what_is_typed_is_shown_whole() {
    let keys = |names: &[&str]| {
        names
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        typed("print(1)\n", &keys(&["Enter", "Ctrl-C"])),
        "print(1) \u{23ce} [Enter] [Ctrl-C]"
    );
    assert_eq!(typed("a\r\nb", &[]), "a \u{23ce} b");
    assert_eq!(typed("", &keys(&["Ctrl-C"])), "[Ctrl-C]");
    assert_eq!(typed("y ", &[]), "y ");
}
