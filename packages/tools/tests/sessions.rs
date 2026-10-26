//! `kill_all_now`, as the signal handler calls it before quitting: in a process of its own,
//! because the table it empties is shared by every session in the process, and the unit tests'
//! sessions run in parallel.
#![cfg(unix)]
#![expect(
    clippy::disallowed_methods,
    reason = "a test looks again until a process is gone, which has no event"
)]

use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use tools::shell::{
    Ending, Sessions, Until, Wait, kill_all_now, kill_with, running_anywhere, running_with,
};

/// The tests empty one table: one at a time.
fn alone() -> MutexGuard<'static, ()> {
    static ONE: Mutex<()> = Mutex::new(());
    ONE.lock().unwrap_or_else(PoisonError::into_inner)
}

#[test]
fn kill_all_now_reaches_more_than_four() {
    let _alone = alone();
    // Two registries, as two sessions of the interface keep: more shells than one may hold.
    let (one, two) = (Sessions::default(), Sessions::default());
    let dir = std::env::temp_dir();
    let wait = Wait {
        needle: Some("started"),
        yield_ms: Some(10_000),
    };
    let mut started = Vec::new();
    for sessions in [&one, &one, &one, &two, &two] {
        let turn = sessions
            .start(
                "echo started; exec sleep 60",
                &dir,
                Default::default(),
                wait,
                None,
                &|| false,
            )
            .unwrap();
        assert_eq!(turn.until, Until::Matched, "{turn:?}");
        started.push((sessions, turn.id));
    }
    let running = running_anywhere();
    assert_eq!(running.len(), 5, "{running:?}");
    assert!(
        running
            .iter()
            .all(|command| command.contains("exec sleep 60"))
    );

    kill_all_now();

    assert_eq!(running_anywhere(), Vec::<String>::new());
    let poll = Wait {
        needle: None,
        yield_ms: Some(2_000),
    };
    for (sessions, id) in started {
        let ended = sessions.send(id, b"", &[], poll, None, &|| false).unwrap();
        assert_eq!(ended.until, Until::Exited(Ending::Signal(9)), "{ended:?}");
    }
}

#[test]
fn kill_all_now_kills_a_live_session() {
    let _alone = alone();
    let sessions = Sessions::default();
    let dir = std::env::temp_dir();
    let wait = Wait {
        needle: Some("started"),
        yield_ms: Some(10_000),
    };
    // `set -m`: job control, as in an interactive shell, so the sleep is in a group of its own and
    // only the sweep of the session's processes reaches it.
    let command = "set -m; sleep 60 & echo started $!; wait";
    let turn = sessions
        .start(command, &dir, Default::default(), wait, None, &|| false)
        .unwrap();
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
            .send(turn.id, b"", &[], poll, None, &|| false)
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
    let ended = sessions
        .send(turn.id, b"", &[], poll, None, &|| false)
        .unwrap();
    assert_eq!(ended.until, Until::Exited(Ending::Signal(9)), "{ended:?}");
}

#[test]
fn the_sessions_of_one_numbering_are_listed_and_killed_apart() {
    let _alone = alone();
    // Two interfaces' lanes: each interface numbers its own lanes' sessions.
    let (mine, theirs) = (Arc::new(AtomicU32::new(0)), Arc::new(AtomicU32::new(0)));
    let one = Sessions::default().numbered(Arc::clone(&mine));
    let two = Sessions::default().numbered(Arc::clone(&mine));
    let other = Sessions::default().numbered(Arc::clone(&theirs));
    let dir = std::env::temp_dir();
    let wait = Wait {
        needle: Some("started"),
        yield_ms: Some(10_000),
    };
    let mut started = Vec::new();
    for (sessions, command) in [(&one, "mine-1"), (&two, "mine-2"), (&other, "theirs")] {
        let command = format!("echo started; exec sleep 60 # {command}");
        let turn =
            (sessions.start(&command, &dir, Default::default(), wait, None, &|| false)).unwrap();
        assert_eq!(turn.until, Until::Matched, "{turn:?}");
        started.push((sessions, turn.id));
    }
    let mut listed = running_with(&mine);
    listed.sort();
    assert_eq!(listed.len(), 2, "{listed:?}");
    assert!(listed[0].contains("mine-1") && listed[1].contains("mine-2"));
    assert!(running_with(&theirs)[0].contains("theirs"));

    kill_with(&mine);

    let poll = Wait {
        needle: None,
        yield_ms: Some(300),
    };
    for (sessions, id) in &started[..2] {
        let ended = sessions.send(*id, b"", &[], poll, None, &|| false).unwrap();
        assert_eq!(ended.until, Until::Exited(Ending::Signal(9)), "{ended:?}");
    }
    let (sessions, id) = started[2];
    let still = sessions.send(id, b"", &[], poll, None, &|| false).unwrap();
    assert_eq!(still.until, Until::Deadline, "theirs runs on: {still:?}");
}
