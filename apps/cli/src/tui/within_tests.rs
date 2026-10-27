use std::process::Command;
use std::time::Duration;

use super::within;

#[cfg(unix)]
#[test]
fn a_child_is_waited_for_until_its_deadline_then_killed() {
    let mut echo = Command::new("sh");
    echo.args(["-c", "echo one; echo two"]);
    let out = within(echo, vec![], Duration::from_secs(10));
    assert_eq!(out.unwrap(), b"one\ntwo\n");
    let mut failed = Command::new("sh");
    failed.args(["-c", "echo half; exit 3"]);
    assert_eq!(within(failed, vec![], Duration::from_secs(10)), None);
    let mut stuck = Command::new("sh");
    stuck.args(["-c", "exec sleep 30"]);
    let asked = std::time::Instant::now();
    assert_eq!(within(stuck, vec![], Duration::from_millis(200)), None);
    assert!(
        asked.elapsed() < Duration::from_secs(5),
        "{:?}",
        asked.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn what_the_child_is_given_reaches_it_even_when_it_never_reads_the_rest() {
    let cat = Command::new("cat");
    let out = within(cat, b"copied".to_vec(), Duration::from_secs(10));
    assert_eq!(out.unwrap(), b"copied");
    let mut deaf = Command::new("sh");
    deaf.args(["-c", "exec sleep 30"]);
    let big = vec![b'x'; 4 << 20];
    assert_eq!(within(deaf, big, Duration::from_millis(200)), None);
}
