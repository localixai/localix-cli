use std::time::Duration;

use super::*;
use crate::server::socket::tests::{daemon, place};
use crate::testing::Scratch;

const PROBE: Duration = Duration::from_millis(50);

#[test]
fn each_config_has_its_own_socket_in_the_persons_own_directory() {
    let home = socket_path(Path::new("/home/a/.config/llx"));
    let again = socket_path(Path::new("/home/a/.config/llx"));
    let test_home = socket_path(Path::new("/tmp/test-home/.config/llx"));
    assert_eq!(home, again);
    assert_ne!(home, test_home);
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    assert_eq!(
        home.parent().unwrap(),
        runtime_dir().join(format!("llx-{uid}"))
    );
    let name = home.file_name().unwrap().to_str().unwrap();
    assert!(name.ends_with(".sock") && name.len() <= 24, "{name}");
}

#[test]
fn a_second_daemon_is_refused_the_lock_until_the_first_lets_go() {
    let scratch = Scratch::new();
    let config = scratch.path().join("config");
    let first = lock(&config).unwrap();
    assert!(first.is_some());
    assert!(
        lock(&config).unwrap().is_none(),
        "two daemons for one config"
    );
    drop(first);
    assert!(lock(&config).unwrap().is_some());
}

#[test]
fn the_probe_answers_only_for_a_daemon_that_answers() {
    let path = place("probe");
    assert!(!probe(&path, PROBE), "nothing there");
    drop(crate::server::socket::bind(&path).unwrap());
    assert!(!probe(&path, PROBE), "a stale socket");
    let _daemon = daemon(&path);
    assert!(probe(&path, PROBE));
}

#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "the desk's requests, as the binary makes its channel"
)]
fn a_daemon_says_its_build_and_who_is_attached() {
    let path = place("status");
    assert_eq!(status(&path, PROBE), None, "nothing there");
    let host = crate::server::host::Host::new(build());
    let served = std::sync::Arc::clone(&host);
    crate::server::socket::accept(crate::server::socket::bind(&path).unwrap(), move |c| {
        let (desk, _requests) = std::sync::mpsc::channel();
        let peer = crate::server::Peer::new(c.out);
        let info = crate::server::serve::Info {
            max_lanes: 1,
            shells: std::sync::Arc::new(Vec::new),
            commands: vec![],
            modes: vec![],
            background: true,
            host: Some(std::sync::Arc::clone(&served)),
        };
        crate::server::serve::serve(&c.incoming, &peer, &desk, &info);
    })
    .unwrap();
    let _attached = host.attach();
    let found = status(&path, Duration::from_secs(5)).unwrap();
    assert_eq!(
        (found.build, found.pid, found.attached),
        (build(), std::process::id(), 1)
    );
    assert!(build().starts_with(env!("CARGO_PKG_VERSION")));
}

#[test]
fn a_daemon_is_idle_only_when_nothing_needs_it() {
    let nothing = Status {
        build: build(),
        pid: 1,
        attached: 0,
        lanes: 0,
        working: 0,
        kept: 0,
        jobs: 0,
    };
    assert!(nothing.idle());
    let with = |set: fn(&mut Status)| {
        let mut status = Status {
            build: build(),
            ..nothing
        };
        set(&mut status);
        status.idle()
    };
    assert!(!with(|s| s.attached = 1), "a terminal attached");
    assert!(!with(|s| s.lanes = 1), "a session open");
    assert!(!with(|s| s.kept = 1), "sessions moved to the background");
    assert!(!with(|s| s.jobs = 1), "a monitor running");
    assert!(!with(|s| s.working = 1), "a task working");
}

/// A daemon that answers `shutdown` only after `late`, then ends: one under load.
#[allow(
    clippy::disallowed_methods,
    reason = "the test's own slow daemon: it waits on purpose before it answers"
)]
fn slow_daemon(path: &Path, late: Duration) {
    use std::io::{BufRead, BufReader, Write};
    let listener = crate::server::socket::bind(path).unwrap();
    let serving = std::thread::Builder::new().name("slow".into());
    serving
        .spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            let call: protocol::ToServer = serde_json::from_str(&line).unwrap();
            let protocol::ToServer::Call { id, .. } = call else {
                panic!("{line}")
            };
            std::thread::sleep(late);
            let done = protocol::ToClient::Done { id, done: Done::Ok };
            let mut out = &stream;
            writeln!(out, "{}", serde_json::to_string(&done).unwrap()).unwrap();
            // The daemon ends: its connection goes with it.
        })
        .unwrap();
}

#[test]
fn a_daemon_slow_to_answer_its_stop_under_load_is_still_stopped() {
    let path = place("slow-stop");
    slow_daemon(&path, Duration::from_secs(3));
    assert!(stop(&path), "a stop answered in 3 s is a stop");
}
