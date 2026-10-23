use super::*;
use crate::shell::keys;

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
fn non_printing_bytes_are_dropped() {
    let cases: [(&[u8], &str); 4] = [
        // Control sequences: colours, erasing, cursor moves.
        (b"\x1b[31mred\x1b[0m\n\x1b[2K\x1b[1;5Hplain\n", "red\nplain"),
        // String sequences, ended by BEL or by ST.
        (
            b"\x1b]0;title\x07a\x1b]8;;http://x\x1b\\b\x1bP1$r0m\x1b\\c\n",
            "abc",
        ),
        // Two-byte escapes.
        (b"\x1b(Bx\x1b=y\x1b>z\n", "xyz"),
        // Bells and other controls; a tab stays.
        (b"a\x07b\x00c\x7fd\te\n", "abcd\te"),
    ];
    for (input, expected) in cases {
        let (out, pending, seen) = filtered(input);
        assert_eq!(out, expected, "{input:?}");
        assert_eq!(pending, "", "{input:?}");
        assert_eq!(seen, format!("{expected}\n"), "{input:?}");
    }
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
fn application_cursor_mode_is_followed() {
    let mut filter = Filter::default();
    let mut out = Output::default();
    let mut seen = vec![];
    // What the Up key sends after each output, as a session presses it.
    let mut up = |bytes: &[u8]| {
        filter.feed(bytes, &mut out, &mut seen);
        keys::encode("Up", filter.application_cursor).unwrap()
    };
    assert_eq!(up(b"\x1b[?1h"), b"\x1bOA");
    assert_eq!(up(b"\x1b[?1049;1l"), b"\x1b[A");
    assert_eq!(up(b"\x1b[?1049h"), b"\x1b[A", "another mode is not DECCKM");
    up(b"\x1b[?");
    assert_eq!(up(b"1h"), b"\x1bOA", "split across reads");
}

#[test]
fn a_query_is_answered() {
    let mut filter = Filter::default();
    let mut out = Output::default();
    let mut seen = vec![];
    filter.feed(b"\x1b[6nx\x1b[5n", &mut out, &mut seen);
    assert_eq!(filter.take_replies(), b"\x1b[0n");
    assert_eq!(
        filter.position_asked, 1,
        "the screen answers where the cursor is"
    );
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
