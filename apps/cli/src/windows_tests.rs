use std::sync::{Arc, Mutex};

use clap::Parser;

use super::*;
use crate::cli::Cli;
use crate::testing::Scratch;

fn options(args: &[&str]) -> RunOptions {
    Cli::try_parse_from([&["llx"], args].concat()).unwrap().run
}

#[test]
fn windows_show_in_a_session_and_not_in_exec_or_over_ssh() {
    let plain = options(&[]);
    assert!(shown(&plain, true, false), "a session");
    assert!(!shown(&plain, false, false), "exec");
    assert!(!shown(&plain, true, true), "over ssh");
    let forced = options(&["--show-browser"]);
    assert!(shown(&forced, false, true), "forced");
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
    let watch: Watch = Arc::new(move |_id: u32, title: &str| {
        seen.lock().unwrap().push(title.to_string());
        Some(Box::new(|_| true) as Painter)
    });
    (watch, titles)
}

/// The titles of the windows a kept `cat` opens, in a session (`asks` false is `exec`), with
/// `show` as its call gives it.
fn opened(asks: bool, remote: bool, show: Option<bool>) -> Vec<String> {
    let (watch, titles) = recording();
    let sessions = terminals(Some(watch), shown(&options(&[]), asks, remote));
    let dir = Scratch::new();
    let wait = tools::shell::Wait {
        needle: None,
        yield_ms: Some(200),
    };
    let turn = sessions
        .start("cat", dir.path(), Default::default(), wait, show, &|| false)
        .unwrap();
    assert!(sessions.close(turn.id).is_ok(), "cat was kept");
    titles.lock().unwrap().clone()
}

#[test]
fn a_kept_session_opens_a_window_in_a_session_only() {
    assert_eq!(opened(true, false, None), ["session 1 · cat"]);
    assert_eq!(opened(true, false, Some(false)), [] as [&str; 0]);
    for (asks, remote) in [(false, false), (true, true)] {
        let case = format!("asks {asks}, ssh {remote}");
        assert_eq!(opened(asks, remote, None), [] as [&str; 0], "{case}");
        // Not even when a call asks for one: nobody is there to see it.
        assert_eq!(opened(asks, remote, Some(true)), [] as [&str; 0], "{case}");
    }
}

#[test]
fn a_lanes_tabs_are_of_its_directory_by_its_name() {
    let root = std::path::Path::new("/Users/me/code/llx");
    assert_eq!(group("/Users/me/code/shop", root), "shop");
    assert_eq!(group("/Users/me/code/shop/", root), "shop");
    assert_eq!(group("", root), "llx");
    assert_eq!(group("/", root), "/");
}
