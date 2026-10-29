use std::time::Duration;

use super::*;

#[test]
fn a_failure_a_wait_may_mend_is_tried_three_times_then_said() {
    let mut tries = 0;
    let result: Result<(), u32> = retried(Duration::ZERO, None, || {
        tries += 1;
        Attempt::Again(tries)
    });
    assert_eq!((result, tries), (Err(3), 3));
}

#[test]
fn an_answer_or_a_lasting_failure_ends_the_attempts() {
    let mut tries = 0;
    let result: Result<u32, ()> = retried(Duration::ZERO, None, || {
        tries += 1;
        if tries == 2 {
            Attempt::Done(Ok(tries))
        } else {
            Attempt::Again(())
        }
    });
    assert_eq!(result, Ok(2));
    let mut tries = 0;
    let result: Result<(), &str> = retried(Duration::ZERO, None, || {
        tries += 1;
        Attempt::Done(Err("refused"))
    });
    assert_eq!((result, tries), (Err("refused"), 1));
}

#[test]
fn no_pause_is_slept_past_the_deadline() {
    let mut tries = 0;
    let started = Instant::now();
    let until = started + Duration::from_millis(100);
    let result: Result<(), u32> = retried(Duration::from_secs(30), Some(until), || {
        tries += 1;
        Attempt::Again(tries)
    });
    assert_eq!((result, tries), (Err(1), 1));
    assert!(started.elapsed() < Duration::from_secs(1));
}
