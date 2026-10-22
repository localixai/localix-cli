use std::cell::RefCell;
use std::io::Cursor;

use super::*;

fn lines(input: &str) -> (Vec<String>, String) {
    raw(input.as_bytes())
}

fn raw(input: &[u8]) -> (Vec<String>, String) {
    let mut input = Cursor::new(input.to_vec());
    let prompts = RefCell::new(String::new());
    let shown = |text: &str| prompts.borrow_mut().push_str(text);
    let mut read_all = vec![];
    while let Some(text) = read(&mut input, &shown) {
        read_all.push(text);
    }
    (read_all, prompts.into_inner())
}

#[test]
fn each_line_is_an_entry_and_the_end_of_input_quits() {
    let (entries, prompts) = lines("fix the test\n\nrun it\r\n");
    assert_eq!(entries, ["fix the test", "", "run it"]);
    assert_eq!(prompts.matches("› ").count(), 4, "one prompt per read");
}

#[test]
fn a_backslash_goes_on_on_the_next_line() {
    let (entries, _) = lines("first part \\\nsecond part\nnext\n");
    assert_eq!(entries, ["first part \nsecond part", "next"]);
}

#[test]
fn a_last_line_without_a_newline_still_counts() {
    assert_eq!(lines("only this").0, ["only this"]);
    assert_eq!(lines("cut \\").0, ["cut \n"]);
}

#[test]
fn slash_commands_are_not_tasks() {
    assert_eq!(entry("   "), None);
    assert_eq!(entry(" /exit "), Some(Entry::Exit));
    assert_eq!(entry("/quit"), Some(Entry::Exit));
    assert_eq!(entry("/help"), Some(Entry::Help));
    assert_eq!(entry("/undo"), Some(Entry::Unknown("undo".into())));
    assert_eq!(
        entry("add a test for /api/users"),
        Some(Entry::Task("add a test for /api/users".into()))
    );
}

#[test]
fn a_line_that_is_not_utf8_is_read_not_taken_for_the_end() {
    let (entries, _) = raw(b"caf\xe9 latin-1\nnext\n");
    assert_eq!(entries, ["caf\u{fffd} latin-1", "next"]);
}

#[test]
fn a_read_error_ends_the_line_and_says_why() {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("the terminal is gone"))
        }
    }
    let mut input = std::io::BufReader::new(Broken);
    let said = RefCell::new(String::new());
    let shown = |text: &str| said.borrow_mut().push_str(text);
    assert_eq!(read(&mut input, &shown), None);
    assert!(
        said.borrow()
            .contains("could not be read: the terminal is gone")
    );
}
