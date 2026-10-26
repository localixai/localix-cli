//! What the command line refuses: exit code 3 and a word on stderr that says why.

use crate::common::home::{FAKE_KEY, Home, text};

#[test]
fn an_unknown_flag_is_a_usage_error() {
    let home = Home::new();
    let missing = home.path().join("missing");
    let cases: &[(&[&str], &str)] = &[
        (&["--no-such-flag", "x"], "--no-such-flag"),
        (&["resume"], "<ID|--last>"),
        // -y is --mode bypass, so it cannot be given with another mode.
        (&["exec", "-y", "--mode", "plan", "x"], "--mode"),
        (&["exec", "-C", missing.to_str().unwrap(), "x"], "-C"),
        (&["exec", "--schema", "[1]", "x"], "--schema"),
        // No prompt, and stdin (closed here) gives none.
        (&["exec"], "no prompt"),
    ];
    for (args, said) in cases {
        let out = home.run_with_key(args);
        assert_eq!(out.status.code(), Some(3), "{args:?}");
        let stderr = text(&out.stderr);
        assert!(stderr.contains(said), "{args:?}: {stderr}");
    }
}

#[test]
fn a_session_without_a_terminal_points_to_exec() {
    let home = Home::new();
    let cases: &[(&[&str], &str)] = &[
        (&["fix the tests"], "llx exec"),
        (&[], "llx exec"),
        (&["--json", "x"], "llx exec"),
        // `llx login` is a session whose first message is "login": it needs a terminal too.
        (&["login", "status"], "needs a terminal"),
    ];
    for (args, said) in cases {
        let out = home.run_with_key(args);
        assert_eq!(out.status.code(), Some(3), "{args:?}");
        let stderr = text(&out.stderr);
        assert!(stderr.contains(said), "{args:?}: {stderr}");
    }
}

/// stdin a terminal, but no size to draw at: it is not the process's own terminal (a session of
/// its own, with none), and stdout is a pipe. The interface cannot start, and there is no plain
/// prompt line to fall back on: a usage error that points to `exec`.
#[cfg(unix)]
#[test]
fn a_session_whose_interface_cannot_start_points_to_exec() {
    use std::process::Stdio;
    let home = Home::new();
    let (master, slave) = crate::common::term::pty();
    let mut command = home.llx(&["fix the tests"]);
    command.current_dir(home.path());
    let lead = || {
        // SAFETY: setsid takes nothing and only makes the child a session's leader.
        unsafe { libc::setsid() };
        Ok(())
    };
    // SAFETY: between fork and exec the closure only calls setsid, which is async-signal-safe and
    // touches only the child.
    unsafe { std::os::unix::process::CommandExt::pre_exec(&mut command, lead) };
    let child = command
        .env("OPENROUTER_API_KEY", FAKE_KEY)
        .stdin(Stdio::from(slave))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Nothing is ever typed: a prompt line reading stdin would end at once.
    drop((command, master));
    let out = child.wait_with_output().unwrap();
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "{stderr}");
    assert!(stderr.contains("llx exec"), "{stderr}");
}

/// A key or setting exported in the shell that runs the tests reaches no test: the binary sees
/// only the home it is given, so no test depends on the machine or calls a real model.
#[cfg(unix)]
#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "a test's fixture: `env` ends by itself"
)]
fn the_binary_sees_nothing_of_the_shell_that_runs_the_tests() {
    let home = Home::new();
    let out = home.command("env").output().unwrap();
    let seen = text(&out.stdout);
    let names: Vec<&str> = seen
        .lines()
        .filter_map(|l| l.split_once('='))
        .map(|(n, _)| n)
        .collect();
    // Cargo exports CARGO_* to every test process: none of it may get through.
    assert!(!names.iter().any(|n| n.starts_with("CARGO")), "{names:?}");
    assert!(
        names.contains(&"PATH") && names.contains(&"HOME"),
        "{names:?}"
    );
    assert!(
        seen.contains("OPENROUTER_BASE_URL=http://127.0.0.1:1\n"),
        "{seen}"
    );
    // Under coverage, the binary writes its profile where the run asked, not into a removed home.
    let profile = std::env::var("LLVM_PROFILE_FILE").ok();
    let passed = seen
        .lines()
        .find_map(|l| l.strip_prefix("LLVM_PROFILE_FILE="));
    assert_eq!(passed, profile.as_deref(), "{seen}");
}

/// The terminal is llx's own, but the interface cannot draw (stdout's reader is gone): the
/// usage error leaves the terminal as it found it, not in raw mode.
#[cfg(unix)]
#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "a test runs the binary to its end, against a scripted model"
)]
fn an_interface_that_cannot_draw_gives_the_terminal_back() {
    use std::os::fd::AsRawFd;
    use std::process::Stdio;
    let home = Home::new();
    let (master, slave) = crate::common::term::pty();
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    let mut command = home.llx(&["fix the tests"]);
    command.current_dir(home.path());
    let lead = || {
        // SAFETY: setsid takes nothing and only makes the child a session's leader.
        unsafe { libc::setsid() };
        // SAFETY: TIOCSCTTY takes no argument; it only sets the session's terminal.
        unsafe { libc::ioctl(0, libc::TIOCSCTTY as _, 0) };
        Ok(())
    };
    // SAFETY: between fork and exec the closure only calls setsid and ioctl, which are
    // async-signal-safe and touch only the child.
    unsafe { std::os::unix::process::CommandExt::pre_exec(&mut command, lead) };
    let out = command
        .env("OPENROUTER_API_KEY", FAKE_KEY)
        .stdin(Stdio::from(slave))
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "{stderr}");
    // The child's session ended with it, which revokes the slave here: the master reads its modes.
    // SAFETY: a termios is integers and flags: all zero is one.
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: tcgetattr fills a termios the call owns, from a descriptor that is open.
    let read = unsafe { libc::tcgetattr(master.as_raw_fd(), &mut termios) };
    assert_eq!(read, 0, "{}", std::io::Error::last_os_error());
    let cooked = libc::ICANON | libc::ECHO;
    assert_eq!(
        termios.c_lflag & cooked,
        cooked,
        "left in raw mode: {stderr}"
    );
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "a test runs the binary to its end, against a scripted model"
)]
fn what_is_refused_points_to_the_command_that_would_do_it() {
    let home = Home::new();
    let here = home.path().display().to_string();
    home.save_session("1700000000-ab12", &here, 1700000000);
    let cases: &[(&[&str], &str)] = &[
        // Only `exec` reads a prompt from stdin: it is the command to give one to.
        (&["exec"], "no prompt: llx exec \"<task>\""),
        (
            &["exec", "resume", "--last"],
            "no prompt: llx exec \"<task>\"",
        ),
        // A resumed session goes on in one task with `exec resume`.
        (&["resume", "--last", "go"], "llx exec resume"),
    ];
    for (args, said) in cases {
        let mut command = home.llx(args);
        let out = command
            .current_dir(home.path())
            .env("OPENROUTER_API_KEY", FAKE_KEY)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        let stderr = text(&out.stderr);
        assert_eq!(out.status.code(), Some(3), "{args:?}: {stderr}");
        assert!(stderr.contains(said), "{args:?}: {stderr}");
    }
}
