//! `kill_all_now`, as the signal handler calls it before quitting: in a process of its own,
//! because the table it empties is shared by every session in the process, and the unit tests'
//! sessions run in parallel.
#![cfg(unix)]

use std::time::{Duration, Instant};

use tools::shell::{Ending, Sessions, Until, Wait, kill_all_now};

#[test]
fn kill_all_now_kills_a_live_session() {
    let sessions = Sessions::default();
    let dir = std::env::temp_dir();
    let wait = Wait {
        needle: Some("started"),
        yield_ms: Some(10_000),
    };
    // `set -m`: job control, as in an interactive shell, so the sleep is in a group of its own and
    // only the sweep of the session's processes reaches it.
    let command = "set -m; sleep 60 & echo started $!; wait";
    let turn = sessions.start(command, &dir, &[], wait, &|| false).unwrap();
    assert_eq!(turn.until, Until::Matched, "{turn:?}");
    let poll = Wait {
        needle: None,
        yield_ms: Some(500),
    };
    let text = match turn
        .output
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
    {
        Some(_) => turn.output.unwrap(),
        None => sessions
            .send(turn.id, b"", &[], poll, &|| false)
            .unwrap()
            .output
            .unwrap(),
    };
    let sleep: i32 = text.split_whitespace().nth(1).unwrap().parse().unwrap();

    kill_all_now();

    // The sleep it started is gone too, reaped by init once orphaned. Looked at before the next
    // call, whose own ending of the session would kill it as well.
    let deadline = Instant::now() + Duration::from_secs(2);
    // SAFETY: signal 0 only asks whether the process exists.
    while unsafe { libc::kill(sleep, 0) } == 0 {
        assert!(Instant::now() < deadline, "the sleep still runs");
        std::thread::sleep(Duration::from_millis(20));
    }
    let ended = sessions.send(turn.id, b"", &[], poll, &|| false).unwrap();
    assert_eq!(ended.until, Until::Exited(Ending::Signal(9)), "{ended:?}");
}
