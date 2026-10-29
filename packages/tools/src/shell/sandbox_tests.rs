use std::time::Duration;

use super::Scope;
use crate::shell::process::{Ending, Env, Finished, Job, run};
use crate::testing::Scratch;

fn read_only(command: &str, dir: &std::path::Path) -> Finished {
    run(&Job {
        command,
        dir,
        timeout: Duration::from_secs(60),
        env: Env::default(),
        stop: &|| false,
        scope: Scope::ReadOnly,
        kept: &[],
    })
    .unwrap()
}

/// The system's word for a refusal: macOS's EPERM, Linux's EACCES (Landlock) or EPERM (seccomp).
fn refused(stderr: &str) -> bool {
    stderr.contains("not permitted") || stderr.contains("Permission denied")
}

#[test]
fn a_read_only_command_reads_and_runs_but_changes_nothing() {
    let dir = Scratch::new();
    let notes = dir.file("notes.txt", "kept");
    let finished = read_only(
        "cat notes.txt; echo changed > notes.txt; rm notes.txt; mkdir made; touch new.txt; \
         mv notes.txt moved.txt; ln -s notes.txt link; echo done",
        dir.path(),
    );
    assert!(finished.stdout.starts_with("kept"), "{}", finished.stdout);
    assert!(finished.stdout.ends_with("done"), "{}", finished.stdout);
    assert!(refused(&finished.stderr), "{}", finished.stderr);
    assert_eq!(std::fs::read_to_string(&notes).unwrap(), "kept");
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().collect();
    assert_eq!(left.len(), 1, "{left:?}");
}

#[test]
fn a_read_only_command_writes_nowhere_outside_either() {
    let dir = Scratch::new();
    let elsewhere = Scratch::new();
    let target = elsewhere.path().join("out.txt");
    let finished = read_only(&format!("echo x > '{}'", target.display()), dir.path());
    assert_ne!(finished.ending, Ending::Exit(0));
    assert!(!target.exists());
}

#[test]
fn a_read_only_command_has_a_scratch_directory_of_its_own_and_dev_null() {
    let dir = Scratch::new();
    let finished = read_only(
        r#"echo kept > "$TMPDIR/x" && cat "$TMPDIR/x" && echo gone > /dev/null && echo "$TMPDIR""#,
        dir.path(),
    );
    assert_eq!(finished.ending, Ending::Exit(0), "{}", finished.stderr);
    let mut lines = finished.stdout.lines();
    assert_eq!(lines.next(), Some("kept"));
    let scratch = lines.next().unwrap_or_default();
    // Removed once the command ended.
    assert!(!std::path::Path::new(scratch).exists(), "{scratch}");
}

#[test]
fn the_scratch_directory_survives_a_clients_environment() {
    // The daemon runs a command with the client's variables, whose own `TMPDIR` is not writable
    // from the sandbox.
    let dir = Scratch::new();
    let vars = [
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("TMPDIR".to_string(), "/nowhere".to_string()),
    ];
    let finished = run(&Job {
        command: r#"echo kept > "$TMPDIR/x" && cat "$TMPDIR/x""#,
        dir: dir.path(),
        timeout: Duration::from_secs(60),
        env: Env {
            vars: Some(&vars),
            hidden: &[],
        },
        stop: &|| false,
        scope: Scope::ReadOnly,
        kept: &[],
    })
    .unwrap();
    assert_eq!(finished.ending, Ending::Exit(0), "{}", finished.stderr);
    assert_eq!(finished.stdout, "kept");
}

#[test]
fn a_read_only_command_opens_no_socket() {
    let dir = Scratch::new();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let finished = read_only(
        &format!(
            "python3 -c \"import socket; socket.create_connection(('127.0.0.1', {port}))\" \
             && echo connected"
        ),
        dir.path(),
    );
    assert!(
        !finished.stdout.contains("connected"),
        "{}",
        finished.stdout
    );
    assert!(refused(&finished.stderr), "{}", finished.stderr);
}

#[test]
fn an_open_command_changes_what_it_says() {
    let dir = Scratch::new();
    let finished = run(&Job {
        command: "echo made > new.txt",
        dir: dir.path(),
        timeout: Duration::from_secs(60),
        env: Env::default(),
        stop: &|| false,
        scope: Scope::Open,
        kept: &[],
    })
    .unwrap();
    assert_eq!(finished.ending, Ending::Exit(0));
    assert!(dir.path().join("new.txt").exists());
}

/// What the app keeps to itself (its keys, its sessions) stays unread, however the command names
/// it; the rest of the same directory reads. Linux's Landlock cannot take a path out of what it
/// lets read: there the workspace's own check is all.
#[cfg(target_os = "macos")]
#[test]
fn a_read_only_command_reads_nothing_the_app_keeps_to_itself() {
    let dir = Scratch::new();
    dir.file("config/credentials", "secret");
    dir.file("notes.txt", "open");
    let kept = [std::fs::canonicalize(dir.path().join("config")).unwrap()];
    let finished = run(&Job {
        command: "cat notes.txt; cat config/credentials; cat ./config/../config/credentials",
        dir: dir.path(),
        timeout: Duration::from_secs(60),
        env: Env::default(),
        stop: &|| false,
        scope: Scope::ReadOnly,
        kept: &kept,
    })
    .unwrap();
    assert!(finished.stdout.starts_with("open"), "{}", finished.stdout);
    assert!(!finished.stdout.contains("secret"), "{}", finished.stdout);
    assert!(refused(&finished.stderr), "{}", finished.stderr);
}
