//! Real processes: jobs a shell with job control put in groups of their own, which a signal to
//! the shell's group never reaches.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Stdio};

use super::*;
use crate::shell::process;
use crate::shell::{Sessions, Until, Wait};
use crate::testing::Scratch;

/// Whether `pid` is gone within two seconds (a killed orphan is reaped by init).
#[expect(
    clippy::disallowed_methods,
    reason = "a test looks again until it is so, for a bounded time: there is no event to wait on"
)]
fn gone(pid: libc::pid_t) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    // SAFETY: signal 0 only asks whether the process exists.
    while unsafe { libc::kill(pid, 0) } == 0 {
        if Instant::now() > deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
    true
}

fn group(pid: libc::pid_t) -> libc::pid_t {
    // SAFETY: getpgid only reads which group a process is in.
    unsafe { libc::getpgid(pid) }
}

/// `command` in a shell that leads a session of its own, as `bash` runs it; the pid it prints
/// first.
fn spawn(command: &str) -> (Child, libc::pid_t) {
    let mut child = process::shell(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    (child, line.trim().parse().unwrap())
}

#[test]
fn a_proc_entry_is_a_pid_only_when_it_is_a_number() {
    assert_eq!(number(b"123\0\0\0"), Some(123));
    assert_eq!(number(b"7"), Some(7));
    assert_eq!(number(b"self\0"), None);
    assert_eq!(number(b"\0"), None);
    assert_eq!(number(b"12a\0"), None);
    assert_eq!(number(b"99999999999999999999\0"), None);
}

#[test]
fn no_session_is_swept_for_init_or_nothing() {
    assert_eq!(kill_members(0), Pass::default());
    assert_eq!(kill_members(1), Pass::default());
}

#[test]
fn the_sweep_kills_the_members_and_spares_the_leader() {
    // The shell reaps the sleep it started once the sweep kills it (a killed child nobody waits
    // for stays a zombie of the session), then waits on its stdin: it outlives the sweep.
    let (mut shell, sleep) = spawn("sleep 300 & echo $!; wait; read line");
    let leader = shell.id().cast_signed();
    assert!(end(leader, Duration::from_secs(2)));
    assert!(gone(sleep), "the sleep still runs");
    assert_eq!(shell.try_wait().unwrap(), None, "the leader was killed");
    shell.stdin.as_mut().unwrap().write_all(b"\n").unwrap();
    shell.wait().unwrap();
}

#[test]
fn a_job_in_a_group_of_its_own_dies_with_its_command() {
    // `set -m`: job control, so the sleep gets a group of its own, as in an interactive shell.
    let (mut shell, sleep) = spawn("set -m; sleep 300 & echo $!; wait");
    let leader = shell.id().cast_signed();
    if group(sleep) == leader {
        eprintln!("this sh has no job control without a terminal: the group kill alone covers it");
    }
    assert!(process::kill(&mut shell), "the kill was not complete");
    assert!(gone(sleep), "the job outlived its command");
}

/// The finding this module fixes: an interactive bash in a session, a job started in the
/// background, then the session closed.
#[expect(
    clippy::disallowed_methods,
    reason = "a test's fixture: the command ends by itself"
)]
#[test]
fn a_background_job_of_an_interactive_shell_dies_with_its_session() {
    let bash = process::shell("command -v bash")
        .stdout(Stdio::null())
        .status();
    if !bash.is_ok_and(|status| status.success()) {
        eprintln!("bash is missing: skipped");
        return;
    }
    let dir = Scratch::new();
    let sessions = Sessions::default();
    let never = || false;
    let ready = Wait {
        needle: None,
        yield_ms: Some(1_000),
    };
    let start = "bash --noprofile --norc -i";
    let turn = sessions
        .start(start, dir.path(), Default::default(), ready, None, &never)
        .unwrap();
    // The echo of what is typed reads `jo''b=$!`: only bash's own output reads `job=<pid>`. The
    // `!` ends the line, where history expansion leaves it alone.
    let input = b"sleep 307 & echo jo''b=$!\n";
    let wait = Wait {
        needle: Some("job="),
        yield_ms: Some(10_000),
    };
    let typed = sessions
        .send(turn.id, input, &[], wait, None, &never)
        .unwrap();
    assert_eq!(typed.until, Until::Matched, "{typed:?}");
    let output = typed.output.unwrap_or_default();
    let pid: libc::pid_t = output
        .split("job=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|pid| pid.parse().ok())
        .unwrap_or_else(|| panic!("{output}"));
    // SAFETY: getsid only reads which session a process is in.
    let session = unsafe { libc::getsid(pid) };
    assert_ne!(group(pid), session, "bash put the job in its own group");
    let closed = sessions.close(turn.id).unwrap();
    assert_eq!(closed.until, Until::Closed { killed: true });
    assert!(gone(pid), "the job outlived its session");
}

/// How many processes the host runs is not the test's to fix, so the table's size is not checked
/// against it; the listing is: this process and a child it just started are both in it.
#[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
#[test]
fn the_listing_holds_this_process_and_its_child() {
    let (mut shell, _) = spawn("echo $$; read line");
    let child = shell.id().cast_signed();
    let mut listed = vec![];
    each_pid(|pid| listed.push(pid));
    shell.stdin.as_mut().unwrap().write_all(b"\n").unwrap();
    shell.wait().unwrap();
    assert!(
        listed.contains(&(std::process::id().cast_signed())),
        "this process"
    );
    assert!(listed.contains(&child), "its child");
}
