//! Unified hunks with bounded LCS work and bounded output. Very large changed regions fall
//! back to removal/insertion rather than allocating a quadratic matrix without a limit.

const CONTEXT: usize = 3;
const MAX_CELLS: usize = 1_000_000;
const MAX_LINES: usize = 200;
const MAX_BYTES: usize = 20_000;
const TRUNCATED: &str = "[Diff truncated: remaining lines/changes omitted.]\n";

type Line<'a> = (char, &'a str);

pub(super) fn unified(before: &str, after: &str) -> String {
    let old: Vec<_> = before.split_inclusive('\n').collect();
    let new: Vec<_> = after.split_inclusive('\n').collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    if prefix == old.len() && prefix == new.len() {
        return String::new();
    }
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut lines: Vec<_> = old[..prefix].iter().map(|line| (' ', *line)).collect();
    changes(
        &old[prefix..old.len() - suffix],
        &new[prefix..new.len() - suffix],
        &mut lines,
    );
    lines.extend(old[old.len() - suffix..].iter().map(|line| (' ', *line)));
    render(&lines)
}

fn changes<'a>(old: &[&'a str], new: &[&'a str], lines: &mut Vec<Line<'a>>) {
    let width = new.len() + 1;
    let cells = (old.len() + 1).checked_mul(width);
    if cells.is_none_or(|cells| cells > MAX_CELLS) {
        lines.extend(old.iter().map(|line| ('-', *line)));
        lines.extend(new.iter().map(|line| ('+', *line)));
        return;
    }
    let mut lcs = vec![0u32; cells.unwrap()];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            lcs[i * width + j] = if old[i] == new[j] {
                1 + lcs[(i + 1) * width + j + 1]
            } else {
                lcs[(i + 1) * width + j].max(lcs[i * width + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        if i < old.len() && j < new.len() && old[i] == new[j] {
            lines.push((' ', old[i]));
            i += 1;
            j += 1;
        } else if i < old.len()
            && (j == new.len() || lcs[(i + 1) * width + j] >= lcs[i * width + j + 1])
        {
            lines.push(('-', old[i]));
            i += 1;
        } else {
            lines.push(('+', new[j]));
            j += 1;
        }
    }
}

fn render(lines: &[Line<'_>]) -> String {
    let mut hunks: Vec<(usize, usize)> = Vec::new();
    for (i, (sign, _)) in lines.iter().enumerate() {
        if *sign == ' ' {
            continue;
        }
        let start = i.saturating_sub(CONTEXT);
        let end = (i + CONTEXT + 1).min(lines.len());
        if let Some(last) = hunks.last_mut().filter(|last| start <= last.1) {
            last.1 = end;
        } else {
            hunks.push((start, end));
        }
    }
    let mut report = String::new();
    let (mut used, mut at, mut old, mut new) = (0, 0, 0, 0);
    for (start, end) in hunks {
        for (sign, _) in &lines[at..start] {
            old += usize::from(*sign != '+');
            new += usize::from(*sign != '-');
        }
        let old_count = lines[start..end]
            .iter()
            .filter(|(sign, _)| *sign != '+')
            .count();
        let new_count = lines[start..end]
            .iter()
            .filter(|(sign, _)| *sign != '-')
            .count();
        let header = format!(
            "@@ -{},{} +{},{} @@\n",
            old + usize::from(old_count != 0),
            old_count,
            new + usize::from(new_count != 0),
            new_count
        );
        if !append(&mut report, &mut used, &header, 1) {
            return report;
        }
        for (sign, line) in &lines[start..end] {
            let unterminated = !line.ends_with('\n');
            // Check before formatting so an enormous source line is never copied.
            let needed = 1 + usize::from(unterminated);
            let extra = if unterminated {
                "\n\\ No newline at end of file\n"
            } else {
                ""
            };
            if report.len() + 1 + line.len() + extra.len() + TRUNCATED.len() > MAX_BYTES {
                report.push_str(TRUNCATED);
                return report;
            }
            if !append(
                &mut report,
                &mut used,
                &format!("{sign}{line}{extra}"),
                needed,
            ) {
                return report;
            }
        }
        old += old_count;
        new += new_count;
        at = end;
    }
    report
}

fn append(report: &mut String, used: &mut usize, text: &str, count: usize) -> bool {
    // Always reserve room for the notice if later hunks exceed the budget.
    if *used + count >= MAX_LINES || report.len() + text.len() + TRUNCATED.len() > MAX_BYTES {
        report.push_str(TRUNCATED);
        return false;
    }
    report.push_str(text);
    *used += count;
    true
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
