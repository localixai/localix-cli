use super::*;

#[test]
fn long_lines_are_cut_with_their_length() {
    let line = "x".repeat(LINE_CHARS + 5);
    let shown = cut(&line, None);
    assert!(shown.ends_with(&format!("{} ... [line cut: 2005 bytes]", "x".repeat(10))));
    assert_eq!(
        shown.len(),
        LINE_CHARS + " ... [line cut: 2005 bytes]".len()
    );
    assert_eq!(cut("abc", Some(9000)), "abc ... [line cut: 9000 bytes]");
    let wide = "\u{e9}".repeat(LINE_CHARS + 1);
    assert_eq!(
        cut(&wide, None).chars().filter(|c| *c == '\u{e9}').count(),
        LINE_CHARS
    );
}
