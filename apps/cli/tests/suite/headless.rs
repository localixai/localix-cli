//! The headless build (ARCHITECTURE.md A8), run with `cargo test -p llx --no-default-features`:
//! it links no browser, so `doctor` and the `window` tool say so plainly, a page cannot open, and a
//! terminal tab goes on working.

use serde_json::json;

use super::exec::exec;
use crate::common::home::{Home, text};
use crate::common::model::{Model, Reply, said};

#[test]
fn doctor_says_the_build_has_no_browser() {
    let home = Home::new();
    let out = home.run(&["doctor"]);
    let shown = text(&out.stdout);
    assert!(
        shown.contains("browser: this build has no browser"),
        "{shown}"
    );
}

#[test]
fn a_page_cannot_open_and_the_model_reads_why_while_a_terminal_tab_works() {
    let home = Home::new();
    let model = Model::start(vec![
        Reply::call(
            "window",
            json!({"action": "open", "kind": "browser", "url": "https://example.com"}),
        ),
        Reply::call(
            "window",
            json!({"action": "open", "kind": "terminal", "command": "echo in a tab; cat"}),
        ),
        Reply::text("Done.\nRESULT: achieved"),
    ]);
    let out = exec(&home, &model, &["exec", "-y", "open a page"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    // The environment block tells it before it tries; the failed open says it again.
    let first = said(&model, 0);
    assert!(
        first.contains("Browser: not available here (this build has no browser)"),
        "{first}"
    );
    let refused = said(&model, 1);
    assert!(
        refused.contains("not opened: pages cannot be opened here: this build has no browser"),
        "{refused}"
    );
    let opened = said(&model, 2);
    assert!(
        opened.contains("t1") && opened.contains("in a tab"),
        "{opened}"
    );
}
