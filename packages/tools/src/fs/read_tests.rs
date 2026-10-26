use serde_json::json;

use super::*;
use crate::testing::Scratch;

#[expect(
    clippy::unwrap_in_result,
    clippy::panic_in_result_fn,
    reason = "a test's helper: a setup that fails, fails the test"
)]
fn read(dir: &Scratch, arguments: Value) -> Result<String, ToolError> {
    let workspace = Workspace::new(dir.path()).unwrap();
    let tool = ReadTool {
        workspace: &workspace,
    };
    tool.call(&arguments).map(|output| {
        assert_eq!(output.state, None);
        output.report
    })
}

fn failed(result: Result<String, ToolError>) -> String {
    match result {
        Err(ToolError::Failed(message)) => message,
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn a_file_comes_back_numbered_with_a_header() {
    let dir = Scratch::new();
    dir.file("src/a.txt", "one\ntwo\r\n\nfour");
    assert_eq!(
        read(&dir, json!({"path": "src/a.txt"})).unwrap(),
        "src/a.txt (lines 1-4 of 4)\n     1\tone\n     2\ttwo\n     3\t\n     4\tfour"
    );
}

#[expect(clippy::format_collect, reason = "a test's fixture, built once")]
#[test]
fn offset_and_limit_pick_a_window_and_say_how_to_go_on() {
    let dir = Scratch::new();
    let text: String = (1..=10).map(|n| format!("line {n}\n")).collect();
    dir.file("ten.txt", text);
    assert_eq!(
        read(&dir, json!({"path": "ten.txt", "offset": 4, "limit": 2})).unwrap(),
        "ten.txt (lines 4-5 of 10)\n     4\tline 4\n     5\tline 5\n\n\
         (lines 6-10 not shown: read again with offset 6 to continue)"
    );
    assert_eq!(
        read(&dir, json!({"path": "ten.txt", "offset": 10})).unwrap(),
        "ten.txt (lines 10-10 of 10)\n    10\tline 10"
    );
    assert_eq!(
        failed(read(&dir, json!({"path": "ten.txt", "offset": 11}))),
        "ten.txt has 10 lines: offset 11 is past the end"
    );
}

#[test]
fn the_default_window_is_two_thousand_lines() {
    let dir = Scratch::new();
    // Empty lines: 2000 of them stay within the byte budget.
    dir.file("long.txt", "\n".repeat(2500));
    let report = read(&dir, json!({"path": "long.txt"})).unwrap();
    assert!(report.starts_with("long.txt (lines 1-2000 of 2500)\n"));
    assert!(report.ends_with("read again with offset 2001 to continue)"));
}

#[expect(clippy::format_collect, reason = "a test's fixture, built once")]
#[test]
fn a_window_stays_within_its_byte_budget() {
    let dir = Scratch::new();
    let text: String = (0..100)
        .map(|_| format!("{}\n", "y".repeat(1000)))
        .collect();
    dir.file("wide.txt", text);
    let report = read(&dir, json!({"path": "wide.txt"})).unwrap();
    assert!(report.len() < MAX_BYTES + 200, "{} bytes", report.len());
    assert!(report.starts_with("wide.txt (lines 1-19 of 100)\n"));
    assert!(report.ends_with("read again with offset 20 to continue)"));
}

#[test]
fn an_overlong_line_is_cut_with_its_length() {
    let dir = Scratch::new();
    dir.file("min.js", format!("{}\nnext\n", "z".repeat(100_000)));
    let report = read(&dir, json!({"path": "min.js"})).unwrap();
    let first = report.lines().nth(1).unwrap();
    assert_eq!(
        first,
        format!(
            "     1\t{} ... [line cut: 100000 bytes]",
            "z".repeat(LINE_CHARS)
        )
    );
    assert!(report.ends_with("     2\tnext"));
}

#[test]
fn an_empty_file_says_so() {
    let dir = Scratch::new();
    dir.file("empty.txt", "");
    assert_eq!(
        read(&dir, json!({"path": "empty.txt"})).unwrap(),
        "empty.txt (empty file)"
    );
}

#[test]
fn a_binary_file_is_refused() {
    let dir = Scratch::new();
    dir.file("image.png", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
    assert_eq!(
        failed(read(&dir, json!({"path": "image.png"}))),
        "image.png is a binary file: it cannot be read as text"
    );
}

#[test]
fn a_missing_file_is_named() {
    let dir = Scratch::new();
    assert_eq!(
        failed(read(&dir, json!({"path": "nope/missing.rs"}))),
        format!("{} does not exist", Path::new("nope/missing.rs").display())
    );
}

#[test]
fn a_directory_lists_its_entries_sorted() {
    let dir = Scratch::new();
    dir.file("b.txt", "");
    dir.file("a/inner.txt", "");
    dir.file("C.md", "");
    assert_eq!(
        read(&dir, json!({"path": "."})).unwrap(),
        "./ (3 entries)\nC.md\na/\nb.txt"
    );
    assert_eq!(
        read(&dir, json!({"path": "a/"})).unwrap(),
        "a/ (1 entry)\ninner.txt"
    );
}

#[test]
fn a_large_directory_is_capped() {
    let dir = Scratch::new();
    for n in 0..MAX_ENTRIES + 3 {
        dir.file(&format!("f{n:04}"), "");
    }
    let report = read(&dir, json!({"path": "."})).unwrap();
    assert_eq!(report.lines().count(), MAX_ENTRIES + 2);
    assert!(
        report
            .ends_with("... 3 more not listed: use bash (`ls`, `find`, `rg --files`) to search it")
    );
}

#[test]
fn unknown_arguments_are_refused() {
    let dir = Scratch::new();
    assert!(failed(read(&dir, json!({"path": "a", "lines": 3}))).starts_with("unusable arguments"));
}

#[test]
fn next_line_keeps_only_the_start_of_a_line() {
    let mut reader = io::Cursor::new(b"abcdef\n\nxy".to_vec());
    let mut line = vec![];
    assert_eq!(next_line(&mut reader, &mut line, 3).unwrap(), Some(6));
    assert_eq!(line, b"abc");
    assert_eq!(next_line(&mut reader, &mut line, 3).unwrap(), Some(0));
    assert_eq!(next_line(&mut reader, &mut line, 3).unwrap(), Some(2));
    assert_eq!(line, b"xy");
    assert_eq!(next_line(&mut reader, &mut line, 3).unwrap(), None);
}

#[expect(
    clippy::disallowed_methods,
    reason = "a test's fixture: the command ends by itself"
)]
#[cfg(unix)]
#[test]
fn what_is_not_a_regular_file_is_refused_before_it_is_opened() {
    let dir = Scratch::new();
    // Opening a FIFO with no writer would block for ever.
    let made = std::process::Command::new("mkfifo")
        .arg(dir.path().join("pipe"))
        .status()
        .unwrap();
    assert!(made.success());
    assert_eq!(
        failed(read(&dir, json!({"path": "pipe"}))),
        "pipe is not a regular file: it cannot be read here"
    );
    // A device that never ends a line, nor the file.
    assert_eq!(
        failed(read(&dir, json!({"path": "/dev/zero"}))),
        "/dev/zero is not a regular file: it cannot be read here"
    );
}
