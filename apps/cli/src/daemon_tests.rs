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
            shells: Vec::new,
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
