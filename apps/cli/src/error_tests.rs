use std::io;

use super::*;

#[test]
fn every_failure_has_its_exit_code() {
    assert_eq!(Failure::Usage("x".into()).code(), exit::ENVIRONMENT);
    assert_eq!(Failure::Environment("x".into()).code(), exit::ENVIRONMENT);
    assert_eq!(Failure::SignedOut.code(), exit::ENVIRONMENT);
    assert_eq!(Failure::Closed.code(), exit::OK);
}

#[test]
fn a_closed_pipe_ends_quietly() {
    let closed = Failure::from_write(&io::Error::from(io::ErrorKind::BrokenPipe));
    assert_eq!(closed, Failure::Closed);
    assert_eq!(closed.message(), None);
    let full = Failure::from_write(&io::Error::from(io::ErrorKind::StorageFull));
    assert!(matches!(full, Failure::Environment(_)));
}

#[test]
fn a_message_names_the_program_and_the_way_out() {
    let message = Failure::SignedOut.message().unwrap();
    assert!(message.starts_with("llx: not signed in"));
    assert!(message.contains("/login"));
    assert_eq!(
        Failure::Usage("the prompt is empty".into()).message(),
        Some("llx: the prompt is empty".into())
    );
}
