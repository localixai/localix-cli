use std::path::PathBuf;

use contracts::gate::{Effect, Open, Verdict};
use serde_json::json;

use super::*;
use crate::testing::{Recorder, Scratch};

fn edit(dir: &Scratch, gate: &dyn Gate, arguments: Value) -> Result<String, ToolError> {
    let workspace = Workspace::new(dir.path()).unwrap();
    let tool = EditTool {
        workspace: &workspace,
        gate,
    };
    tool.call(&arguments).map(|output| output.report)
}

fn failed(result: Result<String, ToolError>) -> String {
    match result {
        Err(ToolError::Failed(message)) => message,
        other => panic!("expected Failed, got {other:?}"),
    }
}

fn change(path: &str, old: &str, new: &str) -> Value {
    json!({"path": path, "old_string": old, "new_string": new})
}

#[test]
fn a_unique_string_is_replaced_and_shown_in_context() {
    let dir = Scratch::new();
    let path = dir.file("src/lib.rs", "a\nb\nc\nd\ne\nf\ng\nh\n");
    let gate = Recorder::default();
    let report = edit(&dir, &gate, change("src/lib.rs", "e\n", "E1\nE2\n")).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "a\nb\nc\nd\nE1\nE2\nf\ng\nh\n"
    );
    let shown = Path::new("src/lib.rs").display().to_string();
    assert_eq!(
        report,
        format!(
            "Edited {shown}: replaced 1 occurrence (+2 -1 lines).\n     2\tb\n     3\tc\n     \
             4\td\n     5\tE1\n     6\tE2\n     7\tf\n     8\tg\n     9\th"
        )
    );
    assert_eq!(
        *gate.asked.borrow(),
        [format!("edit: edit {shown} (+2 -1 lines)")]
    );
}

#[test]
fn a_string_found_several_times_is_refused_with_the_count() {
    let dir = Scratch::new();
    let path = dir.file("x.txt", "foo bar foo baz foo");
    let message = failed(edit(&dir, &Open, change("x.txt", "foo", "qux")));
    assert_eq!(
        message,
        "old_string occurs 3 times in x.txt: add surrounding lines to make it unique, or set \
         replace_all to replace every one"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "foo bar foo baz foo");
}

#[test]
fn replace_all_changes_every_occurrence() {
    let dir = Scratch::new();
    let path = dir.file("x.txt", "foo\nbar\nfoo\n");
    let mut arguments = change("x.txt", "foo", "qux");
    arguments["replace_all"] = json!(true);
    let report = edit(&dir, &Open, arguments).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "qux\nbar\nqux\n");
    assert!(
        report.starts_with("Edited x.txt: replaced 2 occurrences, the first shown (+2 -2 lines).")
    );
}

#[test]
fn a_string_not_found_tells_the_model_to_read_again() {
    let dir = Scratch::new();
    dir.file("x.txt", "    indented\n");
    let message = failed(edit(&dir, &Open, change("x.txt", "\tindented", "x")));
    assert!(message.contains("Read the file again"), "{message}");
    let numbered = failed(edit(
        &dir,
        &Open,
        change("x.txt", "     1\t    indented", "x"),
    ));
    assert!(numbered.contains("line numbers"), "{numbered}");
}

#[test]
fn an_unchanged_replacement_is_refused() {
    let dir = Scratch::new();
    dir.file("x.txt", "same");
    assert!(
        failed(edit(&dir, &Open, change("x.txt", "same", "same"))).contains("nothing to change")
    );
}

#[test]
fn an_empty_old_string_creates_a_file_and_its_directories() {
    let dir = Scratch::new();
    let gate = Recorder::default();
    let report = edit(
        &dir,
        &gate,
        change("new/dir/a.rs", "", "fn a() {}\nfn b() {}\n"),
    )
    .unwrap();
    let shown = Path::new("new/dir/a.rs").display().to_string();
    assert_eq!(report, format!("Created {shown} (2 lines)."));
    assert_eq!(
        fs::read_to_string(dir.path().join("new/dir/a.rs")).unwrap(),
        "fn a() {}\nfn b() {}\n"
    );
    assert_eq!(
        *gate.asked.borrow(),
        [format!("edit: create {shown} (2 lines)")]
    );
}

#[test]
fn an_empty_file_may_be_filled() {
    let dir = Scratch::new();
    let path = dir.file("empty.txt", "");
    edit(&dir, &Open, change("empty.txt", "", "now full")).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "now full");
}

#[test]
fn a_file_with_content_is_never_overwritten_whole() {
    let dir = Scratch::new();
    let path = dir.file("keep.txt", "precious");
    let message = failed(edit(&dir, &Open, change("keep.txt", "", "gone")));
    assert!(
        message.contains("already exists and is not empty"),
        "{message}"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "precious");
}

#[test]
fn a_missing_file_says_how_to_create_it() {
    let dir = Scratch::new();
    let message = failed(edit(&dir, &Open, change("missing.txt", "a", "b")));
    assert!(
        message.contains("does not exist: to create it"),
        "{message}"
    );
}

#[test]
fn files_outside_the_workspace_are_not_changed() {
    let dir = Scratch::new();
    let other = Scratch::new();
    let path = other.file("x.txt", "a");
    let gate = Recorder::default();
    let message = failed(edit(&dir, &gate, change(path.to_str().unwrap(), "a", "b")));
    assert!(message.contains("outside the workspace"), "{message}");
    assert!(failed(edit(&dir, &gate, change("../x.txt", "", "b"))).contains("outside"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "a");
    assert!(gate.asked.borrow().is_empty());
}

#[test]
fn a_denied_edit_changes_nothing_and_says_why() {
    let dir = Scratch::new();
    let path = dir.file("x.txt", "a");
    let gate = Recorder::denying("the person said no");
    assert_eq!(
        failed(edit(&dir, &gate, change("x.txt", "a", "b"))),
        "not allowed: the person said no"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "a");
    assert!(failed(edit(&dir, &gate, change("new.txt", "", "b"))).starts_with("not allowed"));
    assert!(!dir.path().join("new.txt").exists());
}

#[test]
fn crlf_files_keep_their_line_endings() {
    let dir = Scratch::new();
    let path = dir.file("win.txt", "one\r\ntwo\r\nthree\r\n");
    edit(&dir, &Open, change("win.txt", "one\ntwo\n", "1\n2\n")).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "1\r\n2\r\nthree\r\n");
}

#[test]
fn mixed_line_endings_are_matched_as_they_are() {
    let dir = Scratch::new();
    let path = dir.file("mixed.txt", "a\r\nb\nc\n");
    edit(&dir, &Open, change("mixed.txt", "b\nc", "B\nC")).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "a\r\nB\nC\n");
}

#[test]
fn a_directory_or_a_binary_file_is_refused() {
    let dir = Scratch::new();
    dir.file("sub/x.txt", "");
    dir.file("blob.bin", [0xff, 0xfe, 0x00]);
    assert!(failed(edit(&dir, &Open, change("sub", "a", "b"))).contains("is a directory"));
    assert!(failed(edit(&dir, &Open, change("blob.bin", "a", "b"))).contains("not UTF-8"));
}

/// A gate that allows, after writing `content` to `path` as an editor would while the person
/// reads the question.
struct Meddling(PathBuf, &'static str);

impl Gate for Meddling {
    fn check(&self, _: Effect) -> Verdict {
        fs::write(&self.0, self.1).unwrap();
        Verdict::Allow
    }
}

#[test]
fn a_file_changed_while_waiting_for_approval_is_not_overwritten() {
    let dir = Scratch::new();
    let path = dir.file("a.txt", "one\n");
    let gate = Meddling(path.clone(), "one, saved by the editor\n");
    let message = failed(edit(&dir, &gate, change("a.txt", "one", "two")));
    assert_eq!(
        message,
        "a.txt changed while waiting for approval: read it again"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "one, saved by the editor\n"
    );
    let created = dir.path().join("new.txt");
    let gate = Meddling(created.clone(), "written meanwhile\n");
    let message = failed(edit(&dir, &gate, change("new.txt", "", "mine\n")));
    assert!(message.contains("changed while waiting"), "{message}");
    assert_eq!(fs::read_to_string(&created).unwrap(), "written meanwhile\n");
}

#[cfg(unix)]
#[test]
fn a_fifo_is_refused_before_it_is_opened() {
    let dir = Scratch::new();
    let made = std::process::Command::new("mkfifo")
        .arg(dir.path().join("pipe"))
        .status()
        .unwrap();
    assert!(made.success());
    let gate = Recorder::default();
    let message = failed(edit(&dir, &gate, change("pipe", "", "x")));
    assert_eq!(message, "pipe is not a regular file: edit changes files");
    assert!(gate.asked.borrow().is_empty());
}
