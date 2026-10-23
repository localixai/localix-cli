use super::*;

fn key(name: &str) -> Vec<u8> {
    encode(name, false).unwrap_or_else(|| panic!("no key {name:?}"))
}

#[test]
fn named_keys_encode_to_xterm_sequences() {
    // (name, application cursor mode, bytes): only the cursor keys change with the mode.
    let cases: [(&str, bool, &[u8]); 11] = [
        ("Up", false, b"\x1b[A"),
        ("Left", false, b"\x1b[D"),
        ("Home", false, b"\x1b[H"),
        ("Up", true, b"\x1bOA"),
        ("End", true, b"\x1bOF"),
        ("PageUp", true, b"\x1b[5~"),
        ("F1", false, b"\x1bOP"),
        ("f4", false, b"\x1bOS"),
        ("F5", false, b"\x1b[15~"),
        ("F11", false, b"\x1b[23~"),
        ("F12", false, b"\x1b[24~"),
    ];
    for (name, application_cursor, bytes) in cases {
        assert_eq!(
            encode(name, application_cursor).as_deref(),
            Some(bytes),
            "{name} {application_cursor}"
        );
    }
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
