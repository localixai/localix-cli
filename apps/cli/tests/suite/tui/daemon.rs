//! A session at the terminal runs in the daemon (docs/daemon.md): the first `llx` of a home
//! starts `llx serve`, the next attaches to it, `llx serve stop` ends it. A daemon of another
//! build is replaced when idle, and left alone when busy. `--no-daemon` and `exec` never start
//! one.

use std::time::Duration;

use serde_json::json;

use super::LONG;
use crate::common::model::{Model, Reply};
use crate::common::term::{Term, key};

/// What `llx serve status` says in `term`'s home.
fn status(term: &Term) -> String {
    term.llx(&["serve", "status"]).0
}

/// What it says once it says `words`: a client is counted as its desk starts, just after the
/// prompt shows.
#[allow(
    clippy::disallowed_methods,
    reason = "a status has no event to wait on: it is asked again every 50 ms, for a bounded time"
)]
fn status_with(term: &Term, words: &str) -> String {
    let end = std::time::Instant::now() + LONG;
    loop {
        let said = status(term);
        if said.contains(words) || std::time::Instant::now() > end {
            return said;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_session_s_commands_run_in_the_daemon_it_started() {
    let model = Model::start(vec![
        Reply::call("bash", json!({"command": "echo $PPID > parent"})),
        Reply::text("It ran there.\nRESULT: achieved"),
    ]);
    let mut term = Term::start(&model, &["-y"]);
    term.type_text("where do you run").press(key::ENTER);
    term.wait_for("It ran there.", LONG);
    let parent = std::fs::read_to_string(term.file("parent")).unwrap();
    let said = status_with(&term, "1 attached");
    assert!(said.contains(&format!("pid {},", parent.trim())), "{said}");
    assert!(said.contains("1 attached"), "{said}");
}

#[test]
fn a_second_terminal_attaches_to_the_same_daemon() {
    let model = Model::start(vec![]);
    let first = Term::start(&model, &[]);
    let pid = status(&first);
    let second = first.beside(&model, &[]);
    let said = status_with(&second, "2 attached");
    assert!(said.contains("2 attached"), "{said}");
    assert_eq!(pid.split(',').nth(1), said.split(',').nth(1), "one daemon");
}

#[test]
fn serve_stop_ends_the_daemon_and_its_terminals() {
    let model = Model::start(vec![]);
    let mut term = Term::start(&model, &[]);
    let (said, code) = term.llx(&["serve", "stop"]);
    assert_eq!((said.trim(), code), ("llx serve: stopped", 0));
    assert!(term.exited(LONG).is_some(), "{}", term.screen());
    let (said, code) = term.llx(&["serve", "status"]);
    assert_eq!((said.trim(), code), ("llx serve: not running", 1));
}

#[test]
fn no_daemon_and_exec_never_start_one() {
    let model = Model::start(vec![Reply::text("Done.\nRESULT: achieved")]);
    let flag = ("LLX_NO_DAEMON", "1");
    for (args, env) in [(&["--no-daemon"][..], &[][..]), (&[], &[flag])] {
        let term = Term::spawn(&model, args, env);
        term.wait_for("❯", LONG);
        assert!(status(&term).contains("not running"), "{args:?} {env:?}");
    }
    let mut term = Term::spawn(&model, &["exec", "say done"], &[]);
    assert!(term.exited(LONG).is_some(), "{}", term.screen());
    assert!(status(&term).contains("not running"));
}

#[test]
fn an_idle_daemon_of_another_build_is_replaced() {
    let model = Model::start(vec![]);
    let older = Term::spawn(&model, &[], &[("LLX_BUILD", "0.0.0+older")]);
    older.wait_for("❯", LONG);
    assert!(status(&older).contains("build 0.0.0+older"));
    let newer = older.beside(&model, &[]);
    let said = status(&newer);
    assert!(
        said.contains("1 attached") && !said.contains("older"),
        "{said}"
    );
}

#[test]
fn a_busy_daemon_of_another_build_is_left_working() {
    let model = Model::start(vec![Reply::call(
        "bash",
        json!({"command": "for i in $(seq 400); do sleep 0.05; done"}),
    )]);
    let mut older = Term::spawn(&model, &["-y"], &[("LLX_BUILD", "0.0.0+older")]);
    older.wait_for("❯", LONG);
    older.type_text("work a while").press(key::ENTER);
    older.wait_for("Running for i in", LONG);
    let newer = older.beside(&model, &[]);
    let said = String::from_utf8_lossy(&newer.raw()).into_owned();
    assert!(said.contains("this session runs in this process"), "{said}");
    let said = status(&newer);
    assert!(
        said.contains("build 0.0.0+older") && said.contains("1 working"),
        "{said}"
    );
    assert_eq!(
        older.exited(Duration::from_millis(300)),
        None,
        "the older works on"
    );
}

#[test]
fn moved_to_the_background_the_work_goes_on_and_the_next_terminal_shows_it() {
    let wait = "until [ -e go ]; do sleep 0.1; done; echo went";
    let model = Model::start(vec![
        Reply::call("bash", json!({"command": wait})),
        Reply::text("Finished while you were away.\nRESULT: achieved"),
        Reply::text("And this too.\nRESULT: achieved"),
    ]);
    let mut first = Term::start(&model, &["-y"]);
    first.type_text("a long task").press(key::ENTER);
    first.wait_for("Running until", LONG);
    first.press(key::CTRL_D);
    first.wait_for("Move to background and exit", LONG);
    first.type_text("2");
    assert!(first.exited(LONG).is_some(), "the terminal is let go");
    // The task goes on in the daemon with nobody attached, and ends.
    std::fs::write(first.file("go"), "").unwrap();
    let end = std::time::Instant::now() + LONG;
    while model.requests().len() < 2 {
        assert!(std::time::Instant::now() < end, "the task did not go on");
        std::thread::yield_now();
    }
    // The next `llx` here shows the session as it went on.
    let mut second = first.beside(&model, &[]);
    let screen = second.wait_for("Finished while you were away.", LONG);
    assert!(screen.contains("❯ a long task"), "{screen}");
    // And the person goes on in it: the same conversation.
    second.type_text("one more thing").press(key::ENTER);
    second.wait_for("And this too.", LONG);
    let asked = crate::common::model::said(&model, 2);
    assert!(
        asked.contains("a long task") && asked.contains("one more thing"),
        "{asked}"
    );
}
