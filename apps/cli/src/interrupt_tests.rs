use super::*;

#[test]
fn outside_a_task_a_stop_quits() {
    let interrupt = Interrupt::new();
    assert!(interrupt.stop());
}

#[test]
fn in_a_task_the_first_stop_stops_it_and_the_second_quits() {
    let interrupt = Interrupt::new();
    interrupt.begin();
    assert!(!interrupt.stopped());
    assert!(!interrupt.stop(), "the first stop lets the task end itself");
    assert!(interrupt.stopped());
    assert!(interrupt.stop(), "the second quits");
}

#[test]
fn the_next_task_starts_unstopped() {
    let interrupt = Interrupt::new();
    interrupt.begin();
    interrupt.stop();
    interrupt.end();
    assert!(interrupt.stopped(), "until the next task, it says so");
    interrupt.begin();
    assert!(!interrupt.stopped());
}
