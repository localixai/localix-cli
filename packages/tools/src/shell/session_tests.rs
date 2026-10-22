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
        .start(command, dir.path(), &[], wait, &never)
        .unwrap()
}

fn send(sessions: &Sessions, id: u32, input: &str, keys: &[&str], wait: Wait) -> Turn {
    let keys: Vec<String> = keys.iter().map(|key| key.to_string()).collect();
    let typed = super::super::keys::typed(input);
    sessions.send(id, &typed, &keys, wait, &never).unwrap()
}

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
    session.pty.child.id() as i32
}

/// Whether `pid` (or with a minus, a process group) is gone, within two seconds.
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
fn python_input_round_trip() {
    if !python() {
        return;
    }
    let (dir, sessions) = (Scratch::new(), sessions());
    let command = r#"python3 -c 'print("got", input("? "))'"#;
    let turn = start(&sessions, &dir, command, until("?"));
    assert_eq!(turn.until, Until::Matched, "{turn:?}");
    let turn = send(&sessions, turn.id, "x\n", &[], until("got"));
    let turn = match turn.until {
        // The answer and the exit may come in one call or two.
        Until::Matched => send(&sessions, turn.id, "", &[], Wait::default()),
        _ => turn,
    };
    assert_eq!(turn.until, Until::Exited(Ending::Exit(0)), "{turn:?}");
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
fn a_stop_kills_every_session() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let short = Wait {
        needle: None,
        yield_ms: Some(100),
    };
    let first = start(&sessions, &dir, "sleep 30", short);
    let second = start(&sessions, &dir, "sleep 31", short);
    let groups = [pid(&sessions, first.id), pid(&sessions, second.id)];
    let stopped = sessions.send(first.id, b"", &[], short, &|| true).unwrap();
    assert_eq!(stopped.until, Until::Stopped);
    assert_eq!(sessions.running(), "none");
    for group in groups {
        assert!(gone(-group), "group {group} still runs");
    }
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
        .start("sleep 30", dir.path(), &[], short, &never)
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
fn a_bash_session_answers() {
    if Command::new("bash").arg("-c").arg("").status().is_err() {
        return;
    }
    let (dir, sessions) = (Scratch::new(), sessions());
    let turn = start(&sessions, &dir, "bash --noprofile --norc -i", until("$"));
    assert_eq!(turn.until, Until::Matched, "{turn:?}");
    send(&sessions, turn.id, "export V=7\n", &[], until("$"));
    let turn = send(&sessions, turn.id, "echo $V\n", &[], until("7"));
    assert_eq!(turn.until, Until::Matched, "{turn:?}");
}

#[test]
fn colour_and_progress_are_filtered() {
    let (dir, sessions) = (Scratch::new(), sessions());
    let command = r"printf '\033[31mred\033[0m\n10%%\r50%%\r100%%\n'";
    let turn = start(&sessions, &dir, command, Wait::default());
    assert_eq!(turn.until, Until::Exited(Ending::Exit(0)), "{turn:?}");
    assert_eq!(output(&turn), "red\n100%");
}

#[test]
fn a_session_runs_in_its_directory_without_the_hidden_variables() {
    let (dir, sessions) = (Scratch::new(), sessions());
    dir.file("marker.txt", "");
    let command = r#"ls; echo "term=$TERM home=${HOME+set}""#;
    let turn = sessions
        .start(command, dir.path(), &["HOME"], Wait::default(), &never)
        .unwrap();
    assert_eq!(output(&turn), "marker.txt\nterm=dumb home=", "{turn:?}");
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
