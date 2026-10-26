//! Real terminals. The tests that need python3 pass without it, saying so.

use std::process::Command;
use std::thread;
use std::time::Instant;

use super::*;
use crate::testing::Scratch;

/// Each test's own: they are closed when it ends (`Sessions` closes all it has when dropped), so
/// nothing is left behind.
fn sessions() -> Sessions {
    Sessions::default()
}

fn never() -> bool {
    false
}

fn until(needle: &str) -> Wait<'_> {
    Wait {
        needle: Some(needle),
        yield_ms: Some(10_000),
    }
}

fn start(sessions: &Sessions, dir: &Scratch, command: &str, wait: Wait) -> Turn {
    sessions
        .start(command, dir.path(), Default::default(), wait, None, &never)
        .unwrap()
}

fn send(sessions: &Sessions, id: u32, input: &str, keys: &[&str], wait: Wait) -> Turn {
    let keys: Vec<String> = keys.iter().map(|key| key.to_string()).collect();
    let typed = super::super::keys::typed(input);
    sessions
        .send(id, &typed, &keys, wait, None, &never)
        .unwrap()
}

#[allow(
    clippy::disallowed_methods,
    reason = "a test's fixture: the command ends by itself"
)]
fn python() -> bool {
    let found = Command::new("python3").arg("-c").arg("").status().is_ok();
    if !found {
        eprintln!("python3 is missing: skipped");
    }
    found
}

/// The pid of session `id`'s shell, which leads its process group.
fn pid(sessions: &Sessions, id: u32) -> i32 {
    let registry = sessions.registry();
    let session = registry
        .live
        .iter()
        .find(|session| session.id == id)
        .unwrap();
    session.pty.child.id().cast_signed()
}

/// Whether `pid` (or with a minus, a process group) is gone, within two seconds.
#[allow(
    clippy::disallowed_methods,
    reason = "a test looks again until it is so, for a bounded time: there is no event to wait on"
)]
fn gone(pid: i32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        // SAFETY: signal 0 only asks whether the process exists.
        let exists = unsafe { libc::kill(pid, 0) } == 0
            || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH);
        if !exists {
            return true;
        }
        if Instant::now() > deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn output(turn: &Turn) -> &str {
    turn.output.as_deref().unwrap_or("")
}

#[test]
fn a_line_program_reads_and_answers() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let command = r#"printf 'Name: '; read n; echo "hi $n""#;
    let turn = start(&sessions, &dir, command, until("Name:"));
    assert_eq!(turn.until, Until::Matched);
    assert_eq!(output(&turn), "Name:");
    let turn = send(&sessions, turn.id, "bob\n", &[], Wait::default());
    assert_eq!(turn.until, Until::Exited(Ending::Exit(0)), "{turn:?}");
    assert!(output(&turn).contains("hi bob"), "{turn:?}");
    assert_eq!(sessions.running(), "none");
}

#[test]
fn a_repl_keeps_state_across_calls() {
    if !python() {
        return;
    }
    let (dir, sessions) = (Scratch::new(), sessions());
    let turn = start(&sessions, &dir, "python3 -q", until(">>>"));
    assert_eq!(turn.until, Until::Matched, "{turn:?}");
    let id = turn.id;
    let shell = pid(&sessions, id);
    send(&sessions, id, "a = 40\n", &[], until(">>>"));
    let turn = send(&sessions, id, "print(a + 2)", &["Enter"], until("42"));
    assert_eq!(turn.until, Until::Matched, "{turn:?}");
    let turn = sessions.close(id).unwrap();
    assert_eq!(turn.until, Until::Closed { killed: true });
    assert!(gone(shell), "the shell still runs");
    assert!(sessions.command(id).unwrap_err().contains("is not running"));
}

#[test]
fn ctrl_c_interrupts_a_sleep() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let turn = start(
        &sessions,
        &dir,
        "sleep 30",
        Wait {
            needle: None,
            yield_ms: Some(300),
        },
    );
    assert_eq!(turn.until, Until::Deadline);
    let began = Instant::now();
    let turn = send(&sessions, turn.id, "", &["Ctrl-C"], Wait::default());
    assert_eq!(turn.until, Until::Exited(Ending::Signal(2)), "{turn:?}");
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "{:?}",
        began.elapsed()
    );
}

#[test]
fn stdin_and_stdout_are_a_tty() {
    if !python() {
        return;
    }
    let (dir, sessions) = (Scratch::new(), sessions());
    let command = "python3 -c 'import sys; print(sys.stdin.isatty(), sys.stdout.isatty())'";
    let turn = start(&sessions, &dir, command, Wait::default());
    assert_eq!(turn.until, Until::Exited(Ending::Exit(0)), "{turn:?}");
    assert_eq!(output(&turn), "True True");
    assert_eq!(sessions.running(), "none", "an ended command is not kept");
}

/// A descriptor this process has open without close-on-exec when a session starts never reaches
/// it: the program gets its terminal, and nothing else.
#[test]
fn a_session_inherits_only_its_terminal() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let (_stray, probe) = crate::testing::stray();
    let turn = start(&sessions, &dir, &probe, Wait::default());
    assert_eq!(turn.until, Until::Exited(Ending::Exit(0)), "{turn:?}");
    assert_eq!(output(&turn), "closed");
}

#[test]
fn output_between_calls_is_kept() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let command = "sleep 0.5; echo late; sleep 30";
    let turn = start(
        &sessions,
        &dir,
        command,
        Wait {
            needle: None,
            yield_ms: Some(100),
        },
    );
    assert_eq!(turn.until, Until::Deadline);
    assert_eq!(turn.output, None);
    let turn = send(
        &sessions,
        turn.id,
        "",
        &[],
        Wait {
            needle: None,
            yield_ms: Some(2_000),
        },
    );
    assert_eq!(output(&turn), "late", "{turn:?}");
    assert_eq!(turn.until, Until::Settled);
}

#[test]
fn close_kills_what_it_started() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let command = "sh -c 'sleep 60 & echo started $!; wait'";
    let turn = start(&sessions, &dir, command, until("started"));
    let turn = match output(&turn).split_whitespace().nth(1) {
        Some(_) => turn,
        None => send(&sessions, turn.id, "", &[], Wait::default()),
    };
    let text = output(&turn).to_string();
    let child: i32 = text.split_whitespace().nth(1).unwrap().parse().unwrap();
    // SAFETY: signal 0 only asks whether the process exists.
    assert_eq!(unsafe { libc::kill(child, 0) }, 0, "the sleep runs");
    let closed = sessions.close(turn.id).unwrap();
    assert_eq!(closed.until, Until::Closed { killed: true });
    assert!(gone(child), "the sleep it started still runs");
}

#[test]
fn a_stop_ends_the_wait_and_leaves_the_program_running() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let short = Wait {
        needle: None,
        yield_ms: Some(100),
    };
    let first = start(&sessions, &dir, "sleep 30", short);
    let second = start(&sessions, &dir, "sleep 31", short);
    let group = pid(&sessions, first.id);
    let stopped = sessions
        .send(first.id, b"", &[], short, None, &|| true)
        .unwrap();
    assert_eq!(stopped.until, Until::Stopped);
    // Ctrl-C stops the task, not a REPL or a program the person is watching.
    assert_eq!(
        // SAFETY: signal 0 only asks whether the group exists.
        unsafe { libc::kill(-group, 0) },
        0,
        "group {group} was killed"
    );
    assert_eq!(
        sessions.running(),
        format!("{} (`sleep 30`), {} (`sleep 31`)", first.id, second.id)
    );
}

/// Session 1 waited on for `millis` on another thread while `then` runs.
#[allow(
    clippy::disallowed_methods,
    reason = "the other thread's wait is under way before `then` runs"
)]
fn while_busy(sessions: &Sessions, millis: u64, then: impl FnOnce()) {
    thread::scope(|scope| {
        scope.spawn(|| {
            let wait = Wait {
                needle: Some("never printed"),
                yield_ms: Some(millis),
            };
            sessions.send(1, b"", &[], wait, None, &never).unwrap();
        });
        thread::sleep(Duration::from_millis(200));
        then();
    });
}

#[test]
fn a_session_in_a_wait_still_counts_and_is_busy() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let short = Wait {
        needle: None,
        yield_ms: Some(100),
    };
    for _ in 0..MAX_SESSIONS {
        start(&sessions, &dir, "sleep 30", short);
    }
    while_busy(&sessions, 1_500, || {
        let refused = sessions.start(
            "sleep 30",
            dir.path(),
            Default::default(),
            short,
            None,
            &never,
        );
        assert!(refused.unwrap_err().contains("sessions are running"));
        let busy = sessions.send(1, b"", &[], short, None, &never).unwrap_err();
        assert!(busy.contains("session 1 is busy in another call"), "{busy}");
    });
    assert_eq!(sessions.close_all(), MAX_SESSIONS);
}

#[test]
fn at_most_four_run() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let short = Wait {
        needle: None,
        yield_ms: Some(100),
    };
    for _ in 0..MAX_SESSIONS {
        start(&sessions, &dir, "sleep 30", short);
    }
    let refused = sessions
        .start(
            "sleep 30",
            dir.path(),
            Default::default(),
            short,
            None,
            &never,
        )
        .unwrap_err();
    assert_eq!(
        refused,
        "4 sessions are running: 1 (`sleep 30`), 2 (`sleep 30`), 3 (`sleep 30`), 4 (`sleep 30`): \
         close one first (session, close: true)"
    );
    assert_eq!(sessions.close_all(), 4);
    let next = start(&sessions, &dir, "sleep 30", short);
    assert_eq!(next.id, 5, "an id is never given twice");
}

#[test]
fn a_session_runs_in_its_directory_without_the_hidden_variables() {
    let (dir, sessions) = (Scratch::new(), sessions());
    dir.file("marker.txt", "");
    let command = r#"ls; echo "term=$TERM home=${HOME+set}""#;
    let turn = sessions
        .start(
            command,
            dir.path(),
            Env {
                vars: None,
                hidden: &["HOME"],
            },
            Wait::default(),
            None,
            &never,
        )
        .unwrap();
    assert_eq!(
        output(&turn),
        "marker.txt\nterm=xterm-256color home=",
        "{turn:?}"
    );
}

#[test]
fn a_wait_counts_from_its_own_limits() {
    let wait = |needle, yield_ms| Wait { needle, yield_ms }.limit();
    assert_eq!(wait(None, None), Duration::from_millis(2_000));
    assert_eq!(wait(Some("x"), None), Duration::from_millis(15_000));
    assert_eq!(wait(None, Some(1)), Duration::from_millis(MIN_YIELD_MS));
    assert_eq!(
        wait(None, Some(u64::MAX)),
        Duration::from_millis(MAX_YIELD_MS)
    );
}

#[test]
fn registries_that_share_their_numbers_never_give_the_same_id() {
    let dir = Scratch::new();
    let numbers = Arc::new(AtomicU32::new(0));
    let (a, b) = (
        Sessions::default().numbered(Arc::clone(&numbers)),
        Sessions::default().numbered(numbers),
    );
    let short = Wait {
        needle: None,
        yield_ms: Some(100),
    };
    let first = start(&a, &dir, "sleep 30", short);
    let second = start(&b, &dir, "sleep 30", short);
    assert_eq!((first.id, second.id), (1, 2));
}

#[test]
fn a_sessions_screen_is_read_whole_whatever_was_read_before() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let turn = start(
        &sessions,
        &dir,
        "printf 'one\\ntwo\\n'; sleep 30",
        until("two"),
    );
    assert_eq!(output(&turn), "one\ntwo");
    let screen = sessions.screen(turn.id).unwrap();
    assert!(
        screen.starts_with("screen 120×40, cursor at row 3 col 1:\none\ntwo"),
        "{screen}"
    );
    assert!(sessions.screen(99).unwrap_err().contains("not running"));
}

#[test]
fn a_session_runs_with_the_environment_it_is_given() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let vars = [
        (
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_default(),
        ),
        ("LLX_PLACE".to_string(), "client".to_string()),
    ];
    let env = Env {
        vars: Some(&vars),
        hidden: &[],
    };
    let command = r#"echo "place=$LLX_PLACE home=${HOME-none}""#;
    let turn = (sessions.start(command, dir.path(), env, Wait::default(), None, &never)).unwrap();
    assert_eq!(output(&turn), "place=client home=none", "{turn:?}");
}
