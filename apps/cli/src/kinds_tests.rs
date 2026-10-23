use serde_json::json;
use tools::shell::{BashTool, Sessions};
use tools::window::Kind;
use tools::workspace::Workspace;

use super::*;
use crate::testing::Scratch;

fn never() -> bool {
    false
}

#[cfg(unix)]
#[test]
fn a_terminal_tab_opens_takes_input_reads_and_closes_as_its_session() {
    let scratch = Scratch::new();
    let workspace = Workspace::new(scratch.path()).unwrap();
    let sessions = Sessions::default();
    let bash = BashTool {
        workspace: &workspace,
        gate: &contracts::gate::Open,
        hidden: &Vec::new,
        stop: &never,
        sessions: &sessions,
    };
    let terminal = Terminal {
        bash: &bash,
        sessions: &sessions,
        stop: &never,
    };
    let command = "printf 'name? '; read n; echo \"hi $n\"; sleep 30";
    let opened = terminal
        .open(&json!({"command": command, "wait_for": "name?"}))
        .unwrap();
    assert_eq!(opened.id, 1);
    let said = terminal
        .interact(
            1,
            &json!({"input": "Ann", "keys": ["Enter"], "wait_for": "hi Ann"}),
        )
        .unwrap();
    assert!(said.report.contains("hi Ann"), "{}", said.report);
    let read = terminal.snapshot(1).unwrap().state.unwrap();
    assert!(
        read.starts_with("screen 120×40") && read.contains("hi Ann"),
        "{read}"
    );
    assert_eq!(
        terminal.tabs(),
        [(1, format!("session 1 · {command}"), String::new())]
    );
    assert!(terminal.view(1).is_ok());
    assert!(terminal.close(1).unwrap().contains("closed"));
    assert!(terminal.tabs().is_empty());
    // A program that ends within the wait opens no tab, and says what it printed.
    let ended = terminal.open(&json!({"command": "echo done"}));
    assert!(matches!(ended, Err(ToolError::Failed(why)) if why.contains("done")));
}

#[test]
fn a_page_that_moved_a_little_reads_as_its_changes() {
    let before = "### Page (untrusted content)\nShop — https://x/\n- heading \"Cart\"\n- text \"0 items\"\n- button \"Buy\"";
    let after = "### Page (untrusted content)\nShop — https://x/\n- heading \"Cart\"\n- text \"1 item\"\n- button \"Buy\"";
    assert_eq!(
        changes(before, after).as_deref(),
        Some("### Page changes since the last snapshot\n- - text \"0 items\"\n+ - text \"1 item\"")
    );
    assert_eq!(
        changes(after, after).as_deref(),
        Some("### Page changes since the last snapshot\n(no visible change)")
    );
    // A new page is read whole.
    let other: String = (0..60).map(|n| format!("- link \"{n}\"\n")).collect();
    assert_eq!(changes(before, &other), None);
}
