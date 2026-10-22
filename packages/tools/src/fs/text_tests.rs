use super::*;

#[test]
fn lines_are_numbered_like_cat_n() {
    assert_eq!(numbered(7, "fn main() {"), "     7\tfn main() {");
    assert_eq!(numbered(1234567, ""), "1234567\t");
}

#[test]
fn long_lines_are_cut_with_their_length() {
    let line = "x".repeat(LINE_CHARS + 5);
    let shown = numbered(1, &line);
    assert!(shown.ends_with(&format!("{} ... [line cut: 2005 bytes]", "x".repeat(10))));
    assert_eq!(
        shown.len(),
        7 + LINE_CHARS + " ... [line cut: 2005 bytes]".len()
    );
    assert_eq!(cut("abc", Some(9000)), "abc ... [line cut: 9000 bytes]");
    let wide = "\u{e9}".repeat(LINE_CHARS + 1);
    assert_eq!(
        cut(&wide, None).chars().filter(|c| *c == '\u{e9}').count(),
        LINE_CHARS
    );
}

#[test]
fn an_excerpt_numbers_the_lines_it_keeps() {
    let text = "a\nb\nc\nd\n";
    assert_eq!(excerpt(text, 2, 3), "     2\tb\n     3\tc");
    assert_eq!(excerpt(text, 0, 1), "     1\ta");
    assert_eq!(excerpt(text, 3, 99), "     3\tc\n     4\td");
}

#[test]
fn only_files_that_end_every_line_with_crlf_are_crlf() {
    assert!(crlf("a\r\nb\r\n"));
    assert!(!crlf("a\r\nb\n"));
    assert!(!crlf("a\nb\n"));
    assert!(!crlf("no newline"));
}

#[test]
fn changes_count_lines_that_differ() {
    assert_eq!(changed("a\nb\nc", "a\nB\nc"), (1, 1));
    assert_eq!(changed("a\nc", "a\nb\nc"), (1, 0));
    assert_eq!(changed("a\nb\nc", ""), (0, 3));
    assert_eq!(changed("x", "x"), (0, 0));
    assert_eq!(changed("a\na", "a"), (0, 1));
}

#[test]
fn text_copied_with_line_numbers_is_noticed() {
    assert!(has_line_numbers("     3\tfn main() {\n     4\t}"));
    assert!(!has_line_numbers("fn main() {\n}"));
    assert!(!has_line_numbers("3 apples\t"));
    assert!(!has_line_numbers(""));
}
