//! Text as the file tools show and change it: lines numbered like `cat -n`, overlong lines cut,
//! line endings told apart, and how many lines an edit adds and removes.

/// Characters of one line the model reads: a minified bundle or a data line would otherwise fill
/// the whole answer.
pub(super) const LINE_CHARS: usize = 2000;

/// `line` numbered `number`, as `cat -n` prints it: the number right-aligned in six columns, a
/// tab, then the line, cut past [`LINE_CHARS`].
pub(super) fn numbered(number: usize, line: &str) -> String {
    format!("{number:>6}\t{}", cut(line, None))
}

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

/// Lines `first..=last` (1-based) of `text`, numbered, around a change the model should see.
pub(super) fn excerpt(text: &str, first: usize, last: usize) -> String {
    let first = first.max(1);
    text.lines()
        .enumerate()
        .skip(first - 1)
        .take((last + 1).saturating_sub(first))
        .map(|(index, line)| numbered(index + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether every line of `text` ends with `\r\n`: such a file is edited with `\n` and written back
/// with `\r\n`, since the model writes `\n`. A file with mixed endings is matched as it is.
pub(super) fn crlf(text: &str) -> bool {
    let lf = text.matches('\n').count();
    lf > 0 && text.matches("\r\n").count() == lf
}

/// The lines replacing `old` with `new` adds and removes, leaving out the lines both share at the
/// start and the end: what a diff would count.
pub(super) fn changed(old: &str, new: &str) -> (usize, usize) {
    let old: Vec<&str> = old.lines().collect();
    let new: Vec<&str> = new.lines().collect();
    let head = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    (new.len() - head - tail, old.len() - head - tail)
}

/// Whether `text` looks copied from a `read` result with its line numbers: every line starts with
/// spaces, digits and a tab.
pub(super) fn has_line_numbers(text: &str) -> bool {
    text.lines().filter(|line| !line.is_empty()).all(|line| {
        let rest = line.trim_start_matches(' ');
        let after = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        after.len() < rest.len() && after.starts_with('\t')
    }) && !text.trim().is_empty()
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
