//! Text as `read` shows it: overlong lines cut.

/// Characters of one line the model reads: a minified bundle or a data line would otherwise fill
/// the whole answer.
pub(super) const LINE_CHARS: usize = 2000;

/// `line` within [`LINE_CHARS`], with a marker saying how long it was. `whole` is its length in
/// bytes when `line` is only its start (the rest was never read).
pub(super) fn cut(line: &str, whole: Option<usize>) -> String {
    let bytes = whole.unwrap_or(line.len());
    match line.char_indices().nth(LINE_CHARS) {
        Some((at, _)) => format!(
            "{} ... [line cut: {bytes} bytes]",
            line.get(..at).unwrap_or(line)
        ),
        None if whole.is_some() => format!("{line} ... [line cut: {bytes} bytes]"),
        None => line.to_string(),
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
