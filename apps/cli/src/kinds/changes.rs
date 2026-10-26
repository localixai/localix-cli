//! What acting in a tab changed on its page, told against the page the model last read whole: a
//! line diff, each change after the line above it, so that of several equal lines (a game's
//! zeros, a list's repeated items) the one that changed is known by its place.

/// Lines of a page's changes at most; past them, the page is read whole.
const CHANGES: usize = 30;

/// The most line pairs the diff compares (an outline is at most a few hundred lines); past it,
/// the page is read whole.
const PAIRS: usize = 4_000_000;

/// One line of the diff.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Line<'a> {
    Kept(&'a str),
    Gone(&'a str),
    Came(&'a str),
}

/// The page's changes: each `- ` gone and `+ ` new line after the kept line above it (`  `), and
/// `  …` between changes apart. None when there are more than `CHANGES`, or when told so they
/// would be longer than the page itself: then the page is read whole.
pub(super) fn changes(before: &str, after: &str) -> Option<String> {
    let (old, new): (Vec<&str>, Vec<&str>) = (before.lines().collect(), after.lines().collect());
    let lines = diff(&old, &new)?;
    let changed = lines
        .iter()
        .filter(|line| !matches!(line, Line::Kept(_)))
        .count();
    if changed > CHANGES {
        return None;
    }
    let told = match changed {
        0 => "(no visible change)".to_string(),
        _ => told(&lines).join("\n"),
    };
    (told.len() <= after.len() || changed == 0)
        .then(|| format!("### Page changes since the last snapshot\n{told}"))
}

/// The lines that tell the changes: each change, the kept line above it, a mark over a gap.
fn told(lines: &[Line<'_>]) -> Vec<String> {
    let mut out = Vec::new();
    let mut shown: Option<usize> = None;
    for (at, line) in lines.iter().enumerate() {
        let text = match line {
            Line::Kept(_) => continue,
            Line::Gone(text) => format!("- {text}"),
            Line::Came(text) => format!("+ {text}"),
        };
        let above = at
            .checked_sub(1)
            .filter(|&above| matches!(lines.get(above), Some(Line::Kept(_))))
            .filter(|&above| shown != Some(above));
        let first = above.unwrap_or(at);
        if shown.is_some_and(|last| last + 1 < first) {
            out.push("  …".to_string());
        }
        if let Some(Line::Kept(kept)) = above.and_then(|above| lines.get(above)) {
            out.push(format!("  {kept}"));
        }
        out.push(text);
        shown = Some(at);
    }
    out
}

/// The longest common subsequence's diff, gone lines before new ones; None past `PAIRS`.
fn diff<'a>(old: &[&'a str], new: &[&'a str]) -> Option<Vec<Line<'a>>> {
    let (n, m) = (old.len(), new.len());
    if n.saturating_mul(m) > PAIRS {
        return None;
    }
    // common[i * (m + 1) + j]: the longest common run of old[i..] and new[j..].
    let width = m + 1;
    let mut common = vec![0u32; (n + 1) * width];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            common[i * width + j] = match old[i] == new[j] {
                true => common[(i + 1) * width + j + 1] + 1,
                false => common[(i + 1) * width + j].max(common[i * width + j + 1]),
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut lines = Vec::with_capacity(n.max(m));
    while i < n || j < m {
        if i < n && j < m && old[i] == new[j] {
            lines.push(Line::Kept(old[i]));
            (i, j) = (i + 1, j + 1);
        } else if j == m || (i < n && common[(i + 1) * width + j] >= common[i * width + j + 1]) {
            lines.push(Line::Gone(old[i]));
            i += 1;
        } else {
            lines.push(Line::Came(new[j]));
            j += 1;
        }
    }
    Some(lines)
}

#[cfg(test)]
#[path = "changes_tests.rs"]
mod tests;
