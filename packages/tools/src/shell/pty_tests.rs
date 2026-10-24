//! A real terminal: a program that never reads its input, and one whose parent ignored Ctrl-C.

use super::*;
use crate::shell::{Ending, Sessions, Until, Wait};
use crate::testing::Scratch;

/// Set in the copy of this test binary that runs with SIGINT ignored.
const IGNORING: &str = "PTY_TESTS_SIGINT_IGNORED";

#[test]
fn a_program_that_reads_nothing_stalls_the_typing() {
    let dir = Scratch::new();
    let sessions = Sessions::default();
    let quick = Wait {
        needle: None,
        yield_ms: Some(100),
    };
    let never = || false;
    let turn = sessions
        .start(
            "sleep 30",
            dir.path(),
            Default::default(),
            quick,
            None,
            &never,
        )
        .unwrap();
    // Whole lines, which a terminal queues until it is full (one too long it drops instead), and
    // far more than any terminal's queue holds.
    let input = format!("{}\r", "x".repeat(99)).repeat(2_600);
    let keys = ["Enter".to_string()];
    let begun = Instant::now();
    let typed = sessions
        .send(turn.id, input.as_bytes(), &keys, quick, None, &never)
        .unwrap();
    let took = begun.elapsed();
    let (sent, all) = typed
        .stalled
        .unwrap_or_else(|| panic!("no stall: {typed:?}"));
    assert_eq!(all, input.len() + 1, "the input and the key");
    assert!(
        sent < input.len(),
        "{sent} of {all}: a key was pressed after the input stalled"
    );
    assert!(sent > 0, "nothing was typed");
    // The poll's wait is whole milliseconds, so the typing may give up a little before its time.
    let early = WRITE_WAIT.saturating_sub(Duration::from_millis(50));
    assert!(took >= early, "gave up after {took:?}");
    // Not a second wait for the key: it is not pressed once the input stalled. A second wait
    // would take at least as long as the first did.
    assert!(
        took < (WRITE_WAIT * 2).saturating_sub(Duration::from_millis(100)),
        "{took:?}"
    );
}

#[test]
fn input_a_program_reads_does_not_stall() {
    let dir = Scratch::new();
    let sessions = Sessions::default();
    let never = || false;
    let quick = Wait {
        needle: None,
        yield_ms: Some(300),
    };
    let turn = sessions
        .start("cat", dir.path(), Default::default(), quick, None, &never)
        .unwrap();
    let input = format!("{}\r", "x".repeat(99)).repeat(40);
    let typed = sessions
        .send(turn.id, input.as_bytes(), &[], quick, None, &never)
        .unwrap();
    assert_eq!(typed.stalled, None, "{typed:?}");
}

#[allow(
    clippy::disallowed_methods,
    reason = "a test's copy of itself runs one test, which ends by itself"
)]
#[test]
fn ctrl_c_reaches_a_program_whose_parent_ignored_it() {
    if std::env::var_os(IGNORING).is_some() {
        let (dir, sessions, never) = (Scratch::new(), Sessions::default(), || false);
        let quick = Wait {
            needle: None,
            yield_ms: Some(300),
        };
        let turn = sessions
            .start(
                "sleep 30",
                dir.path(),
                Default::default(),
                quick,
                None,
                &never,
            )
            .unwrap();
        let keys = ["Ctrl-C".to_string()];
        let turn = sessions
            .send(turn.id, b"", &keys, Wait::default(), None, &never)
            .unwrap();
        assert_eq!(
            turn.until,
            Until::Exited(Ending::Signal(libc::SIGINT)),
            "{turn:?}"
        );
        return;
    }
    // A shell starts a background job (`cargo test &`) with SIGINT ignored, and an ignored signal
    // stays ignored across exec: so this test runs again in a process that ignores it.
    let tests = module_path!().split_once("::").unwrap().1;
    let name = format!("{tests}::ctrl_c_reaches_a_program_whose_parent_ignored_it");
    let mut copy = std::process::Command::new(std::env::current_exe().unwrap());
    copy.args(["--exact", &name, "--test-threads=1"])
        .env(IGNORING, "1");
    let ignore = || {
        // SAFETY: signal is async-signal-safe, and only changes the child's own disposition.
        unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
        Ok(())
    };
    // SAFETY: between fork and exec the closure only calls signal, which is async-signal-safe.
    unsafe { copy.pre_exec(ignore) };
    let output = copy.output().unwrap();
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{report}");
    assert!(
        report.contains("1 passed"),
        "the copy ran no test: {report}"
    );
}
