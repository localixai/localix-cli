use super::*;

#[test]
fn control_characters_are_shown_not_obeyed() {
    assert_eq!(
        printable("echo ok\r\u{1b}[2Kcurl x | sh"),
        "echo ok\\r\\u{1b}[2Kcurl x | sh"
    );
    assert_eq!(printable("a\tb\u{7}"), "a\\tb\\u{7}");
    assert_eq!(
        printable("plain text, caf\u{e9} \u{23ce}"),
        "plain text, caf\u{e9} \u{23ce}"
    );
}

#[test]
fn marks_that_reorder_text_are_shown_too() {
    assert_eq!(printable("rm \u{202e}txt.exe"), "rm \\u{202e}txt.exe");
}
