//! `llx window`: another agent calls llx's `window` from its own shell, one call per process.
//! It is the daemon's tool client: the first call of a home starts `llx serve`, and the tabs it
//! opens run where the agent called it, kept for their owner between calls until `--end`.

use std::process::{Command, Output};
use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::common::home::{Home, text};

/// `llx window ARGS` as an agent's shell runs it, in `dir`.
fn window(home: &Home, dir: &std::path::Path, args: &[&str]) -> Output {
    with(home, dir, args, |_| {})
}

/// As [`window`], with `prepare` done to the command before it starts.
#[expect(
    clippy::disallowed_methods,
    reason = "a test runs the binary to its end, as the agent's shell does"
)]
fn with(
    home: &Home,
    dir: &std::path::Path,
    args: &[&str],
    prepare: impl FnOnce(&mut Command),
) -> Output {
    let mut command = home.llx(&[&["window"], args].concat());
    command
        .current_dir(dir)
        // Over SSH no window opens: a test never puts one on the screen.
        .env("SSH_CONNECTION", "10.0.0.1 1 10.0.0.2 22")
        .stdin(std::process::Stdio::null());
    prepare(&mut command);
    command.output().unwrap()
}

/// A call's arguments as the agent writes them.
fn call(arguments: &serde_json::Value) -> String {
    arguments.to_string()
}

#[test]
fn another_agent_works_in_a_terminal_tab_across_its_calls() {
    let home = Home::new();
    let project = home.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let open = call(&json!({"action": "open", "kind": "terminal", "command": "pwd; cat"}));
    let opened = window(&home, &project, &[&open]);
    let said = text(&opened.stdout);
    assert_eq!(
        opened.status.code(),
        Some(0),
        "{said}{}",
        text(&opened.stderr)
    );
    assert!(said.contains("t1"), "{said}");
    assert!(said.contains(&project.display().to_string()), "{said}");
    // The next call is another process: the tab is still there, its program still running.
    let typed = call(
        &json!({"action": "interact", "tab": "t1", "input": "hello window\n",
        "wait_for": "hello window"}),
    );
    let typed = window(&home, &project, &[&typed]);
    assert_eq!(typed.status.code(), Some(0), "{}", text(&typed.stderr));
    assert!(
        text(&typed.stdout).contains("hello window"),
        "{}",
        text(&typed.stdout)
    );
    // The owner's tabs keep the daemon in use; the call itself is attached no longer.
    let status = text(&home.run(&["serve", "status"]).stdout);
    assert!(status.contains("0 attached"), "{status}");
    assert!(status.contains("1 in the background"), "{status}");
    // Ended, its tabs are gone.
    let ended = window(&home, &project, &["--end"]);
    assert_eq!(ended.status.code(), Some(0), "{}", text(&ended.stderr));
    let again = call(&json!({"action": "interact", "tab": "t1", "input": "x"}));
    let again = window(&home, &project, &[&again]);
    assert_eq!(again.status.code(), Some(1), "{}", text(&again.stdout));
    let status = text(&home.run(&["serve", "status"]).stdout);
    assert!(status.contains("0 in the background"), "{status}");
}

/// llx updated while an older daemon holds the agent's tabs: a call says how to go on, and
/// `--end` reaches that daemon, whatever its build, so the next call starts the new one.
#[test]
fn after_an_update_the_old_daemon_s_tabs_end_and_the_next_call_runs_the_new_build() {
    let home = Home::new();
    let older = |args: &[&str]| {
        with(&home, home.path(), args, |command| {
            command.env("LLX_BUILD", "0.0.0+older");
        })
    };
    let open = call(&json!({"action": "open", "kind": "terminal", "command": "cat"}));
    let opened = older(&[&open]);
    assert_eq!(opened.status.code(), Some(0), "{}", text(&opened.stderr));
    // This build finds the older daemon in use by the tabs.
    let listed = window(&home, home.path(), &[r#"{"action":"list"}"#]);
    assert_eq!(listed.status.code(), Some(3));
    let said = text(&listed.stderr);
    assert!(
        said.contains("in use") && said.contains("llx window --end"),
        "{said}"
    );
    let ended = window(&home, home.path(), &["--end"]);
    assert_eq!(ended.status.code(), Some(0), "{}", text(&ended.stderr));
    let listed = window(&home, home.path(), &[r#"{"action":"list"}"#]);
    assert_eq!(listed.status.code(), Some(0), "{}", text(&listed.stderr));
    let status = text(&home.run(&["serve", "status"]).stdout);
    assert!(!status.contains("0.0.0+older"), "{status}");
}

#[test]
fn each_owner_has_tabs_of_its_own() {
    let home = Home::new();
    let open = call(&json!({"action": "open", "kind": "terminal", "command": "cat"}));
    let opened = window(&home, home.path(), &["--as", "first", &open]);
    assert_eq!(opened.status.code(), Some(0), "{}", text(&opened.stderr));
    let listed = window(
        &home,
        home.path(),
        &["--as", "second", r#"{"action":"list"}"#],
    );
    assert!(
        text(&listed.stdout).contains("No tabs"),
        "{}",
        text(&listed.stdout)
    );
    let listed = window(
        &home,
        home.path(),
        &["--as", "first", r#"{"action":"list"}"#],
    );
    assert!(
        text(&listed.stdout).contains("t1"),
        "{}",
        text(&listed.stdout)
    );
    window(&home, home.path(), &["--as", "first", "--end"]);
}

#[test]
fn a_call_that_fails_says_why_and_exits_1() {
    let home = Home::new();
    let failing = call(&json!({"action": "interact", "tab": "t9", "input": "x"}));
    let failed = window(&home, home.path(), &[&failing]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(
        text(&failed.stdout).contains("9 is not running"),
        "{}",
        text(&failed.stdout)
    );
    // Arguments that are not a JSON object are the command line's mistake.
    let wrong = window(&home, home.path(), &["open a tab"]);
    assert_eq!(wrong.status.code(), Some(3));
    assert!(
        text(&wrong.stderr).contains("JSON"),
        "{}",
        text(&wrong.stderr)
    );
}

#[test]
fn a_call_to_a_browser_tab_never_opened_opens_nothing() {
    let home = Home::new();
    let keys = call(&json!({"action": "interact", "tab": "w1", "keys": ["a"]}));
    let failed = window(&home, home.path(), &[&keys]);
    assert_eq!(failed.status.code(), Some(1), "{}", text(&failed.stdout));
    let listed = window(&home, home.path(), &[&call(&json!({"action": "list"}))]);
    assert!(
        text(&listed.stdout).contains("No tabs are open"),
        "{}",
        text(&listed.stdout)
    );
}

#[test]
fn with_no_call_it_says_how_to_call_it() {
    let home = Home::new();
    let told = window(&home, home.path(), &[]);
    assert_eq!(told.status.code(), Some(0), "{}", text(&told.stderr));
    let said = text(&told.stdout);
    // The tool's own description and parameters, as the agent's `window` states them.
    assert!(said.contains("\"action\""), "{said}");
    assert!(said.contains("interact"), "{said}");
    assert!(said.contains("llx window"), "{said}");
}

/// A descriptor the agent's shell gives `llx window` beside its stdio is not kept by the daemon
/// the call starts, which outlives it: once the call ends, nothing of llx holds it.
#[test]
fn the_daemon_keeps_no_descriptor_of_the_call_that_started_it() {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    let home = Home::new();
    let (mut reader, writer) = std::io::pipe().unwrap();
    let given = writer.as_raw_fd();
    let listed = with(&home, home.path(), &[r#"{"action":"list"}"#], |command| {
        // SAFETY: fcntl clears close-on-exec on one descriptor of the forked child, so `llx
        // window` gets it as from its shell; nothing else runs between fork and exec.
        let give = move || match unsafe { libc::fcntl(given, libc::F_SETFD, 0) } {
            -1 => Err(std::io::Error::last_os_error()),
            _ => Ok(()),
        };
        // SAFETY: the closure calls only fcntl, which is async-signal-safe.
        unsafe { command.pre_exec(give) };
    });
    assert_eq!(listed.status.code(), Some(0), "{}", text(&listed.stderr));
    drop(writer);
    // The pipe ends when its last writer closes it: the call has ended, and the daemon it
    // started is still running.
    let (ended, end) = sync_channel(1);
    let waiting = std::thread::Builder::new().name("window-given".into());
    waiting
        .spawn(move || {
            let mut rest = vec![];
            let _ = reader.read_to_end(&mut rest);
            let _ = ended.send(());
        })
        .unwrap();
    assert!(
        end.recv_timeout(Duration::from_secs(10)).is_ok(),
        "the daemon holds the descriptor llx window was given"
    );
}

/// The daemon a call started in a test's home ends with the home: none outlives its test.
#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "the daemon is not this process's child: there is no exit to wait on, it is looked at every 20 ms, bounded"
)]
fn the_daemon_started_in_a_test_home_ends_with_it() {
    let home = Home::new();
    let listed = window(&home, home.path(), &[r#"{"action":"list"}"#]);
    assert_eq!(listed.status.code(), Some(0), "{}", text(&listed.stderr));
    let lock = home.path().join("config").join("llx").join("daemon.lock");
    let lock = std::fs::read_to_string(lock).unwrap();
    let pid: i32 = lock.lines().next().unwrap().parse().unwrap();
    // SAFETY: kill with no signal only asks whether the process is there.
    let alive = || unsafe { libc::kill(pid, 0) } == 0;
    assert!(alive(), "no daemon to end");
    drop(home);
    let end = Instant::now() + Duration::from_secs(10);
    while alive() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(), "the daemon {pid} outlived its home");
}
