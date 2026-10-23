use std::sync::{Arc, Mutex};

use clap::Parser;

use super::*;
use crate::cli::Cli;
use crate::testing::Scratch;

fn options(args: &[&str]) -> RunOptions {
    Cli::try_parse_from([&["llx"], args].concat()).unwrap().run
}

#[test]
fn windows_show_in_a_session_and_not_in_exec_autonomous_or_over_ssh() {
    let plain = options(&[]);
    for mode in [Mode::Default, Mode::Plan, Mode::Bypass] {
        assert!(shown(&plain, true, mode, false), "{mode:?}");
    }
    assert!(!shown(&plain, false, Mode::Default, false), "exec");
    assert!(!shown(&plain, true, Mode::Autonomous, false), "autonomous");
    assert!(!shown(&plain, true, Mode::Default, true), "over ssh");
    let forced = options(&["--show-browser"]);
    assert!(shown(&forced, false, Mode::Autonomous, true), "forced");
}

#[test]
fn ssh_is_told_by_either_of_its_variables() {
    let set = |name: &'static str| move |asked: &str| asked == name;
    assert!(remote(set("SSH_CONNECTION")));
    assert!(remote(set("SSH_TTY")));
    assert!(!remote(set("DISPLAY")));
}

/// A watch that records the titles it was asked to open, and paints nowhere.
fn recording() -> (Watch, Arc<Mutex<Vec<String>>>) {
    let titles = Arc::new(Mutex::new(vec![]));
    let seen = Arc::clone(&titles);
    let watch: Watch = Arc::new(move |title: &str| {
        seen.lock().unwrap().push(title.to_string());
        Some(Box::new(|_| true) as Painter)
    });
    (watch, titles)
}

/// The titles of the windows a kept `cat` opens, in a session started as `mode` (`asks` false is
/// `exec`), with `show` as its call gives it.
fn opened(asks: bool, mode: Mode, remote: bool, show: Option<bool>) -> Vec<String> {
    let (watch, titles) = recording();
    let sessions = terminals(Some(watch), shown(&options(&[]), asks, mode, remote));
    let dir = Scratch::new();
    let wait = tools::shell::Wait {
        needle: None,
        yield_ms: Some(200),
    };
    let turn = sessions
        .start("cat", dir.path(), &[], wait, show, &|| false)
        .unwrap();
    assert!(sessions.close(turn.id).is_ok(), "cat was kept");
    titles.lock().unwrap().clone()
}

#[test]
fn a_kept_session_opens_a_window_in_a_session_only() {
    assert_eq!(
        opened(true, Mode::Default, false, None),
        ["session 1 · cat"]
    );
    assert_eq!(
        opened(true, Mode::Default, false, Some(false)),
        [] as [&str; 0]
    );
    for (asks, mode, remote) in [
        (false, Mode::Default, false),
        (true, Mode::Autonomous, false),
        (true, Mode::Default, true),
    ] {
        let case = format!("asks {asks}, {mode:?}, ssh {remote}");
        assert_eq!(opened(asks, mode, remote, None), [] as [&str; 0], "{case}");
        // Not even when a call asks for one: nobody is there to see it.
        assert_eq!(
            opened(asks, mode, remote, Some(true)),
            [] as [&str; 0],
            "{case}"
        );
    }
}
