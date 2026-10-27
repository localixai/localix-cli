//! Text from the model or a tool, made safe and flat where the server shows it itself (an approval at the
//! terminal, a session's title and list): control characters and reordering marks are escaped.
//! The interface keeps its own (`tui::printable`): it knows nothing of the server (L7).

/// `text` on one line: its runs of white space (newlines too) each one space, its ends bare.
pub(crate) fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` with every control character and every reordering mark escaped.
pub(crate) fn printable(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let reorders = matches!(
            c,
            '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        );
        match c.is_control() || reorders {
            true => out.extend(c.escape_default()),
            false => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[path = "printable_tests.rs"]
mod tests;
