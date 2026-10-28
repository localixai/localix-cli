//! Where a hunk's old lines are in a file: exact first, then with less and less strictness, as a
//! model that copies a file's lines slips: trailing spaces, indentation, typographic punctuation.

/// The index of the first run of `lines` at or after `start` that is `pattern`. With `eof`, the
/// end of the file is tried first. An empty pattern is found where it starts.
pub(super) fn seek(lines: &[String], pattern: &[String], start: usize, eof: bool) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }
    let last = lines.len().checked_sub(pattern.len())?;
    // An end-of-file pattern is looked for at the end first, then anywhere.
    let starts = [if eof { last.max(start) } else { start }, start];
    let same: [fn(&str, &str) -> bool; 4] = [
        |a, b| a == b,
        |a, b| a.trim_end() == b.trim_end(),
        |a, b| a.trim() == b.trim(),
        |a, b| plain(a) == plain(b),
    ];
    starts.iter().find_map(|&from| {
        same.iter().find_map(|same| {
            (from..=last).find(|&at| {
                lines
                    .get(at..at + pattern.len())
                    .is_some_and(|run| run.iter().zip(pattern).all(|(a, b)| same(a, b)))
            })
        })
    })
}

/// `line` trimmed, its typographic dashes, quotes and spaces made the ASCII ones.
fn plain(line: &str) -> String {
    line.trim()
        .chars()
        .map(|c| match c {
            '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
            '\u{2018}'..='\u{201B}' => '\'',
            '\u{201C}'..='\u{201F}' => '"',
            '\u{00A0}' | '\u{2002}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => ' ',
            other => other,
        })
        .collect()
}

#[cfg(test)]
#[path = "seek_tests.rs"]
mod tests;
