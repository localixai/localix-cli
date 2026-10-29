use std::time::Duration;

use protocol::Status;

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
#[expect(
    clippy::disallowed_methods,
    reason = "a backoff while a child of another test holds the lock between fork and exec"
)]
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
    // A command another test runs at the same time starts by fork, then exec (`pre_exec` rules out
    // posix_spawn): between the two the child holds every descriptor of this process, the lock's
    // among them, so the lock can stay taken for an instant after `first` lets go.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let again = loop {
        match lock(&config).unwrap() {
            Some(held) => break Some(held),
            None if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            None => break None,
        }
    };
    assert!(
        again.is_some(),
        "the lock stayed taken after its holder let go"
    );
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
#[expect(
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
            commands: vec![],
            modes: vec![],
            host: Some(std::sync::Arc::clone(&served)),
        };
        crate::server::serve::serve(None, &c.incoming, &peer, &desk, &info);
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
            ..nothing.clone()
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
#[expect(
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

/// A daemon that ends as soon as it reads `shutdown`, its connection gone before any answer.
fn quick_daemon(path: &Path) {
    use std::io::{BufRead, BufReader};
    let listener = crate::server::socket::bind(path).unwrap();
    let serving = std::thread::Builder::new().name("quick".into());
    serving
        .spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
        })
        .unwrap();
}

#[test]
fn a_daemon_that_ends_before_it_answers_its_stop_is_stopped() {
    let path = place("quick-stop");
    quick_daemon(&path);
    assert!(stop(&path), "a daemon gone is a daemon stopped");
}

#[test]
fn a_client_finds_the_daemon_by_its_lock_whatever_its_own_runtime_directory() {
    let scratch = Scratch::new();
    let config = scratch.path().join("config");
    // Nobody holds it: where this process would put it.
    assert_eq!(socket_path(&config), own_socket(&config));
    let held = lock(&config).unwrap().unwrap();
    assert_eq!(socket_path(&config), own_socket(&config));
    // A daemon started where `$TMPDIR` was another says so in its lock: a client of a host
    // that sets another `$TMPDIR` goes to it, not to a second daemon the lock refuses.
    std::fs::write(
        config.join("daemon.lock"),
        "4242\n/elsewhere/llx-501/a.sock",
    )
    .unwrap();
    assert_eq!(
        socket_path(&config),
        PathBuf::from("/elsewhere/llx-501/a.sock")
    );
    drop(held);
    // Let go: the lock names nothing.
    assert_eq!(socket_path(&config), own_socket(&config));
}
