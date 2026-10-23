use super::*;

#[test]
fn text_split_across_reads_is_found() {
    let mut stream = Stream::default();
    stream.expect(Some("ready>"), true);
    stream.take_in(b"not yet\r\nrea");
    assert!(!stream.matched);
    stream.take_in(b"dy> ");
    assert!(stream.matched);
    assert_eq!(stream.take().as_deref(), Some("not yet\nready>"));
}

#[test]
fn only_what_comes_after_the_input_counts() {
    let mut stream = Stream::default();
    stream.take_in(b"done\r\n");
    stream.expect(Some("done"), true);
    assert!(!stream.matched, "the output before the input");
    stream.take_in(b"done again\r\n");
    assert!(stream.matched, "the output after the input");
}

#[test]
fn a_read_counts_the_output_not_yet_taken() {
    let mut stream = Stream::default();
    stream.take_in(b"added 3 packages\r\n");
    stream.expect(Some("added"), false);
    assert!(stream.matched);
    assert_eq!(stream.take().as_deref(), Some("added 3 packages"));
    stream.expect(Some("added"), false);
    assert!(!stream.matched, "taken already");
    stream.expect(Some(""), false);
    assert!(!stream.matched, "empty text is no condition");
}

#[test]
fn a_read_finds_text_the_line_not_yet_ended_begins() {
    let mut stream = Stream::default();
    stream.take_in(b"Successfully insta");
    // An earlier call took the output while the line was being printed.
    assert_eq!(stream.take().as_deref(), Some("Successfully insta"));
    stream.expect(Some("Successfully installed"), false);
    assert!(!stream.matched, "not there yet");
    stream.take_in(b"lled foo\r\n");
    assert!(stream.matched, "the needle began in the pending line");
}

#[test]
fn typing_does_not_count_the_line_it_types_after() {
    let mut stream = Stream::default();
    stream.take_in(b"rea");
    stream.expect(Some("ready"), true);
    stream.take_in(b"dy");
    assert!(!stream.matched, "the needle began before the input");
}

#[test]
fn the_prompt_shows_again_with_what_was_typed() {
    let mut stream = Stream::default();
    stream.take_in(b"Name: ");
    assert_eq!(stream.take().as_deref(), Some("Name:"));
    assert_eq!(stream.take(), None, "nothing new");
    stream.take_in(b"bob\r\nhi bob\r\n");
    assert_eq!(stream.take().as_deref(), Some("Name: bob\nhi bob"));
}

#[test]
fn only_control_sequences_are_no_output() {
    let mut stream = Stream::default();
    stream.take_in(b"\x1b[?25l\x1b[0m");
    assert_eq!(stream.take(), None);
}

#[test]
fn a_query_is_answered_through_the_reader() {
    let mut stream = Stream::default();
    assert_eq!(stream.take_in(b"\x1b[6n"), b"\x1b[1;1R");
    stream.take_in(b"\x1b[?1h");
    assert!(stream.application_cursor());
}
