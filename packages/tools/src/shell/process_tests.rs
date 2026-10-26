use super::*;
use crate::testing::Scratch;

const LONG: Duration = Duration::from_secs(30);

fn never() -> bool {
    false
}

fn job<'a>(command: &'a str, dir: &'a Path, timeout: Duration) -> Job<'a> {
    Job {
        command,
        dir,
        timeout,
        env: Env::default(),
        stop: &never,
    }
}

#[cfg(unix)]
#[test]
fn a_command_returns_its_code_and_both_streams() {
    let dir = Scratch::new();
    let finished = run(&job("echo out; echo err 1>&2; exit 3", dir.path(), LONG)).unwrap();
    assert_eq!(finished.ending, Ending::Exit(3));
    assert_eq!(finished.stdout, "out");
    assert_eq!(finished.stderr, "err");
    assert!(!finished.open);
}

#[test]
fn hidden_variables_never_reach_the_command() {
    let dir = Scratch::new();
    // A variable the test process always has, and the shell does not need to start.
    let (name, command) = if cfg!(windows) {
        (
            "USERPROFILE",
            "if defined USERPROFILE (echo seen) else (echo hidden)",
        )
    } else {
        (
            "HOME",
            "if [ -n \"${HOME+x}\" ]; then echo seen; else echo hidden; fi",
        )
    };
    let shown = run(&job(command, dir.path(), LONG)).unwrap();
    assert_eq!(shown.stdout, "seen");
    let hidden = run(&Job {
        env: Env {
            vars: None,
            hidden: &[name],
        },
        ..job(command, dir.path(), LONG)
    })
    .unwrap();
    assert_eq!(hidden.stdout, "hidden");
}

#[cfg(unix)]
#[test]
fn a_command_has_no_terminal_to_prompt_on() {
    let dir = Scratch::new();
    let finished = run(&job(
        "( : </dev/tty ) 2>/dev/null && echo terminal || echo none; echo \"$GIT_TERMINAL_PROMPT\"",
        dir.path(),
        LONG,
    ))
    .unwrap();
    assert_eq!(finished.stdout, "none\n0");
}

#[cfg(unix)]
#[test]
fn a_timeout_kills_the_command_and_what_it_started() {
    let dir = Scratch::new();
    let started = Instant::now();
    // The shell waits on a child, which holds the pipes too: both must die for the call to end.
    let finished = run(&job(
        "sleep 5; echo never",
        dir.path(),
        Duration::from_millis(300),
    ))
    .unwrap();
    assert_eq!(finished.ending, Ending::TimedOut { killed: true });
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    assert!(finished.elapsed >= Duration::from_millis(300));
    assert_eq!(finished.stdout, "");
    assert!(!finished.open);
}

#[cfg(windows)]
#[test]
fn a_timeout_kills_the_command_on_windows_too() {
    let dir = Scratch::new();
    let started = Instant::now();
    let finished = run(&job(
        "ping -n 6 127.0.0.1 >NUL",
        dir.path(),
        Duration::from_millis(300),
    ))
    .unwrap();
    assert_eq!(finished.ending, Ending::TimedOut { killed: true });
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    assert!(!finished.open);
}

#[expect(
    clippy::disallowed_methods,
    reason = "a test looks again until it is so, for a bounded time: there is no event to wait on; kill -0 ends by itself"
)]
#[cfg(unix)]
#[test]
fn a_stop_kills_the_command_and_what_it_started() {
    let dir = Scratch::new();
    let started = Instant::now();
    let stop = || started.elapsed() >= Duration::from_millis(200);
    let finished = run(&Job {
        stop: &stop,
        ..job("sleep 30 & echo $! > child.pid; wait", dir.path(), LONG)
    })
    .unwrap();
    assert_eq!(finished.ending, Ending::Stopped { killed: true });
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    let pid = std::fs::read_to_string(dir.path().join("child.pid")).unwrap();
    let alive = || {
        Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    };
    // The orphan is reaped by init shortly after it dies.
    let deadline = Instant::now() + Duration::from_secs(2);
    while alive() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(), "the background sleep outlived the stop");
}

/// A command's end is noticed when its output closes, not at the next look of a poll: let end
/// right after the runner looked at it (as a poll would then sleep its longest), it is told at
/// once.
#[cfg(unix)]
#[test]
fn a_command_s_end_is_noticed_at_once() {
    let dir = Scratch::new();
    let fifo = dir.path().join("go");
    let made = Command::new("mkfifo").arg(&fifo).spawn().unwrap().wait();
    assert!(made.unwrap().success());
    let started = Instant::now();
    let ended: Mutex<Option<Instant>> = Mutex::new(None);
    let look = || {
        let mut ended = ended.lock().unwrap();
        if ended.is_none() && started.elapsed() > Duration::from_millis(300) {
            // Its reader is waiting on it already: opening it for writing does not wait.
            std::fs::write(&fifo, "x\n").unwrap();
            *ended = Some(Instant::now());
        }
        false
    };
    let finished = run(&Job {
        stop: &look,
        ..job("read x < go", dir.path(), LONG)
    })
    .unwrap();
    let noticed = ended.lock().unwrap().unwrap().elapsed();
    assert_eq!(finished.ending, Ending::Exit(0));
    assert!(
        noticed < Duration::from_millis(25),
        "noticed {noticed:?} after it ended"
    );
}

#[cfg(unix)]
#[test]
fn a_signal_is_told_apart_from_an_exit() {
    let dir = Scratch::new();
    let finished = run(&job("kill -s TERM $$", dir.path(), LONG)).unwrap();
    assert_eq!(finished.ending, Ending::Signal(15));
}

#[cfg(unix)]
#[test]
fn a_background_process_holding_the_output_is_not_waited_for() {
    let dir = Scratch::new();
    let started = Instant::now();
    let finished = run(&job("echo up; sleep 20 &", dir.path(), LONG)).unwrap();
    assert_eq!(finished.ending, Ending::Exit(0));
    // Well before the sleep ends, with room for a loaded machine.
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(finished.stdout, "up");
    assert!(finished.open);
}

#[test]
fn much_output_on_both_streams_never_stalls() {
    let dir = Scratch::new();
    let command = if cfg!(windows) {
        "for /L %i in (1,1,20000) do @(echo line %i & echo line %i 1>&2)"
    } else {
        "i=0; while [ $i -lt 20000 ]; do echo line $i; echo line $i 1>&2; i=$((i+1)); done"
    };
    let finished = run(&job(command, dir.path(), LONG)).unwrap();
    assert_eq!(finished.ending, Ending::Exit(0));
    assert!(finished.stdout.contains("characters cut"));
    assert!(finished.stderr.contains("characters cut"));
}
