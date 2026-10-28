//! Esc Esc rewinds the code and the conversation to before a task.

use serde_json::json;

use super::LONG;
use crate::common::model::{Model, Reply, said};
use crate::common::term::{Term, key};

#[test]
fn a_task_changes_a_file_and_esc_esc_rewinds_it() {
    let model = Model::start(vec![
        Reply::call(
            "edit",
            json!({"path": "notes.txt", "old_string": "", "new_string": "hello\n"}),
        ),
        Reply::text("Created notes.txt.\nRESULT: achieved"),
    ]);
    let mut term = Term::start(&model, &["-y"]);
    term.type_text("create notes.txt").press(key::ENTER);
    term.wait_for("Worked for", LONG);
    assert_eq!(
        std::fs::read_to_string(term.file("notes.txt")).unwrap(),
        "hello\n"
    );
    // The model was offered the script that reads, and the patch that changes the files.
    let tools = model.requests()[0]["tools"].to_string();
    assert!(tools.contains("\"exec\"") && tools.contains("\"apply_patch\""));
    assert!(said(&model, 0).contains("Goal: create notes.txt"));
    term.press(key::ESC).press(key::ESC);
    term.wait_for("Rewind to before", LONG);
    term.press(key::ENTER);
    term.wait_for("Code and conversation", LONG);
    term.press(key::ENTER);
    let screen = term.wait_for("Rewound", LONG);
    assert!(!term.file("notes.txt").exists(), "{screen}");
    // The task is back in the prompt: the composer's row, not the history's copy of it.
    term.wait_for_cursor_row("❯ create notes.txt", LONG);
}
