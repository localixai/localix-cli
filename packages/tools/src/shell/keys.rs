//! Named keys as the bytes a terminal sends for them, and typed text as a terminal types it. The
//! names are strict: literal text goes in `input`, so what the person approves is unambiguous.

/// The names a key may have, for the message that refuses another.
pub(super) const NAMES: &str = "Enter, Tab, Escape, Backspace, Space, Up, Down, Left, Right, Home, End, \
PageUp, PageDown, Delete, F1-F12, or Ctrl-<letter>";

/// The bytes of the key called `name`, or `None` when no key is called that. With
/// `application_cursor` (a program turned DECCKM on) the cursor keys are sent as SS3, as a
/// terminal would.
pub(super) fn encode(name: &str, application_cursor: bool) -> Option<Vec<u8>> {
    let lower = name.trim().to_ascii_lowercase();
    if let Some(control) = control(&lower) {
        return Some(vec![control]);
    }
    let plain: String = lower
        .chars()
        .filter(|c| !matches!(c, ' ' | '_' | '-' | '+'))
        .collect();
    let cursor = |letter: u8| match application_cursor {
        true => vec![0x1b, b'O', letter],
        false => vec![0x1b, b'[', letter],
    };
    let bytes: &[u8] = match plain.as_str() {
        "enter" | "return" => b"\r",
        "tab" => b"\t",
        "shifttab" | "backtab" => b"\x1b[Z",
        "escape" | "esc" => b"\x1b",
        "backspace" => b"\x7f",
        "space" => b" ",
        "delete" | "del" => b"\x1b[3~",
        "insert" | "ins" => b"\x1b[2~",
        "pageup" | "pgup" => b"\x1b[5~",
        "pagedown" | "pgdn" | "pgdown" => b"\x1b[6~",
        "up" => return Some(cursor(b'A')),
        "down" => return Some(cursor(b'B')),
        "right" => return Some(cursor(b'C')),
        "left" => return Some(cursor(b'D')),
        "home" => return Some(cursor(b'H')),
        "end" => return Some(cursor(b'F')),
        "f1" => b"\x1bOP",
        "f2" => b"\x1bOQ",
        "f3" => b"\x1bOR",
        "f4" => b"\x1bOS",
        "f5" => b"\x1b[15~",
        "f6" => b"\x1b[17~",
        "f7" => b"\x1b[18~",
        "f8" => b"\x1b[19~",
        "f9" => b"\x1b[20~",
        "f10" => b"\x1b[21~",
        "f11" => b"\x1b[23~",
        "f12" => b"\x1b[24~",
        _ => return None,
    };
    Some(bytes.to_vec())
}

/// `ctrl-c`, `ctrl+c`, `ctrl c`, `ctrlc`: the control character of a letter or of `@[\]^_`. The
/// separators are stripped by hand here, because `_` is both a separator and a key.
fn control(lower: &str) -> Option<u8> {
    let rest = lower.strip_prefix("ctrl")?;
    let separated = rest.strip_prefix(['-', '+', ' ', '_']).unwrap_or(rest);
    let key = [separated, rest].into_iter().find(|key| key.len() == 1)?;
    let key = key.as_bytes()[0];
    (key.is_ascii_lowercase() || b"@[\\]^_".contains(&key)).then(|| key.to_ascii_uppercase() & 0x1f)
}

/// `input` as a terminal types it: a line break is Enter (`\r`), which the terminal turns back
/// into `\n` for a program that reads lines.
pub(super) fn typed(input: &str) -> Vec<u8> {
    input.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
