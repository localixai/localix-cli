use super::*;

/// `input` fed all at once, and fed one byte at a time: the transcript, the pending line and what
/// was seen must not depend on how the reads were cut.
fn filtered(input: &[u8]) -> (String, String, String) {
    let whole = run(input, input.len().max(1));
    assert_eq!(whole, run(input, 1), "split one byte per read");
    whole
}

fn run(input: &[u8], chunk: usize) -> (String, String, String) {
    let mut filter = Filter::default();
    let mut out = Output::default();
    let mut seen = vec![];
    for part in input.chunks(chunk) {
        filter.feed(part, &mut out, &mut seen);
    }
    let pending = String::from_utf8_lossy(filter.pending_line()).to_string();
    let seen = String::from_utf8_lossy(&seen).to_string();
    (out.text(), pending, seen)
}

#[test]
fn control_sequences_are_dropped() {
    let (out, pending, seen) = filtered(b"\x1b[31mred\x1b[0m\n\x1b[2K\x1b[1;5Hplain\n");
    assert_eq!(out, "red\nplain");
    assert_eq!(pending, "");
    assert_eq!(seen, "red\nplain\n");
}

#[test]
fn string_sequences_are_dropped_with_either_terminator() {
    let (out, ..) = filtered(b"\x1b]0;title\x07a\x1b]8;;http://x\x1b\\b\x1bP1$r0m\x1b\\c\n");
    assert_eq!(out, "abc");
    let (out, ..) = filtered(b"\x1b(Bx\x1b=y\x1b>z\n");
    assert_eq!(out, "xyz");
}

#[test]
fn a_carriage_return_redraws_the_line() {
    let (out, ..) = filtered(b"one\r\ntwo\r\n10%\r50%\r100%\r\n");
    assert_eq!(out, "one\ntwo\n100%");
    // The redraw only begins with the next character: a lone `\r` at the end keeps the line.
    let (_, pending, _) = filtered(b"loading\r");
    assert_eq!(pending, "loading");
}

#[test]
fn backspace_removes_a_whole_character() {
    let (out, ..) = filtered("caf\u{e9}\x08e\nab\x08 \x08\n".as_bytes());
    assert_eq!(out, "cafe\na");
}

#[test]
fn bells_and_other_controls_are_dropped() {
    let (out, ..) = filtered(b"a\x07b\x00c\x7fd\te\n");
    assert_eq!(out, "abcd\te");
}

#[test]
fn application_cursor_mode_is_followed() {
    let mut filter = Filter::default();
    let mut out = Output::default();
    let mut seen = vec![];
    filter.feed(b"\x1b[?1h", &mut out, &mut seen);
    assert!(filter.application_cursor);
    filter.feed(b"\x1b[?1049;1l", &mut out, &mut seen);
    assert!(!filter.application_cursor);
    filter.feed(b"\x1b[?1049h", &mut out, &mut seen);
    assert!(!filter.application_cursor, "another mode is not DECCKM");
    filter.feed(b"\x1b[?", &mut out, &mut seen);
    filter.feed(b"1h", &mut out, &mut seen);
    assert!(filter.application_cursor, "split across reads");
}

#[test]
fn a_query_is_answered() {
    let mut filter = Filter::default();
    let mut out = Output::default();
    let mut seen = vec![];
    filter.feed(b"\x1b[6nx\x1b[5n", &mut out, &mut seen);
    assert_eq!(filter.take_replies(), b"\x1b[1;1R\x1b[0n");
    assert!(filter.take_replies().is_empty());
    assert_eq!(filter.pending_line(), b"x");
}

#[test]
fn an_endless_line_is_flushed_and_stays_bounded() {
    let mut filter = Filter::default();
    let mut out = Output::default();
    let mut seen = vec![];
    for _ in 0..100 {
        filter.feed(&[b'x'; 1000], &mut out, &mut seen);
        assert!(filter.pending_line().len() < HALF);
    }
    let text = out.text();
    assert!(text.len() < 2 * HALF + 100, "{} bytes", text.len());
    assert!(text.contains(" characters cut ..."));
}

#[test]
fn the_prompt_waits_in_the_pending_line() {
    let (out, pending, _) = filtered(b"Welcome\r\nName: ");
    assert_eq!(out, "Welcome");
    assert_eq!(pending, "Name: ");
}
