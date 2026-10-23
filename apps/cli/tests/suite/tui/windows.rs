//! Windows show where somebody watches. llx asks its terminal for its colours, which the terminal
//! sessions' windows take, only when those windows will show: in a session, where a display is
//! (on macOS always; on Linux the test clears `DISPLAY`). A session in autonomous mode or over
//! SSH, and `exec`, ask nothing and open no window, whatever a call asks.

use super::LONG;
use crate::common::model::{Model, Reply};
use crate::common::term::Term;

/// The question for the background colour, which only a windowed run writes.
const ASKED: &[u8] = b"\x1b]11;?";

fn asked(term: &Term) -> bool {
    term.raw().windows(ASKED.len()).any(|bytes| bytes == ASKED)
}

#[test]
fn only_a_watched_session_asks_for_the_terminal_s_colours() {
    let model = Model::start(vec![]);
    let watched = Term::start(&model, &[]);
    assert_eq!(asked(&watched), cfg!(target_os = "macos"), "a session");
    drop(watched);
    let ssh = [("SSH_CONNECTION", "10.0.0.2 52000 10.0.0.1 22")];
    for (args, env) in [(&["--mode", "autonomous"][..], &[][..]), (&[], &ssh)] {
        let term = Term::spawn(&model, args, env);
        term.wait_for("❯", LONG);
        assert!(!asked(&term), "{args:?} {env:?}");
    }
}

#[test]
fn exec_asks_nothing_and_opens_no_window() {
    let model = Model::start(vec![Reply::text("Done.\nRESULT: achieved")]);
    let mut term = Term::spawn(&model, &["exec", "say done"], &[]);
    assert!(term.exited(LONG).is_some(), "{}", term.screen());
    assert!(term.screen().contains("Done."), "{}", term.screen());
    assert!(!asked(&term));
}
