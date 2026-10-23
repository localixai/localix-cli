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
