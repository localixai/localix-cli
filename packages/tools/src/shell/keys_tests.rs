use super::*;

fn key(name: &str) -> Vec<u8> {
    encode(name, false).unwrap_or_else(|| panic!("no key {name:?}"))
}

#[test]
fn arrows_follow_the_cursor_mode() {
    assert_eq!(key("Up"), b"\x1b[A");
    assert_eq!(key("Left"), b"\x1b[D");
    assert_eq!(key("Home"), b"\x1b[H");
    assert_eq!(encode("Up", true).unwrap(), b"\x1bOA");
    assert_eq!(encode("End", true).unwrap(), b"\x1bOF");
    // Only the cursor keys change with the mode.
    assert_eq!(encode("PageUp", true).unwrap(), b"\x1b[5~");
}

#[test]
fn spellings_of_one_key_are_equal() {
    for name in ["PageUp", "Page Up", "page_up", "page-up", "PgUp"] {
        assert_eq!(key(name), b"\x1b[5~", "{name}");
    }
    for name in ["Shift+Tab", "shift-tab", "ShiftTab", "BackTab"] {
        assert_eq!(key(name), b"\x1b[Z", "{name}");
    }
    for name in ["ctrl-c", "Ctrl+C", "CTRL C", "ctrlc"] {
        assert_eq!(key(name), [0x03], "{name}");
    }
    assert_eq!(key("Ctrl-D"), [0x04]);
    assert_eq!(key("Ctrl-["), [0x1b]);
    assert_eq!(key("Ctrl-_"), [0x1f]);
    assert_eq!(key("enter"), b"\r");
    assert_eq!(key("Return"), b"\r");
    assert_eq!(key("Esc"), b"\x1b");
    assert_eq!(key("Backspace"), b"\x7f");
}

#[test]
fn function_keys() {
    assert_eq!(key("F1"), b"\x1bOP");
    assert_eq!(key("f4"), b"\x1bOS");
    assert_eq!(key("F5"), b"\x1b[15~");
    assert_eq!(key("F11"), b"\x1b[23~");
    assert_eq!(key("F12"), b"\x1b[24~");
}

#[test]
fn an_unknown_name_is_no_key() {
    for name in ["Foo", "F13", "Ctrl-1", "Ctrl-Shift-C", "", "a", "Alt-x"] {
        assert_eq!(encode(name, false), None, "{name}");
    }
}

#[test]
fn a_line_break_is_typed_as_enter() {
    assert_eq!(typed("a\r\nb\nc"), b"a\rb\rc");
    assert_eq!(typed("print(1)\n"), b"print(1)\r");
}
