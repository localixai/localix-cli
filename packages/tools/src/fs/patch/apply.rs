//! A file's new content from its old one and a patch's chunks. Lines the patch leaves alone keep
//! their text and their endings, even when they were matched loosely; lines it adds take the
//! file's first ending (`\n` for a file without one), and a changed file ends with a newline.

use super::Chunk;
use super::seek::{nearest, seek};

/// A run of the file's lines replaced by others.
struct Edit<'a> {
    at: usize,
    len: usize,
    with: &'a [String],
}

/// `content` with every chunk applied, each looked for after the one before it. `shown` names the
/// file in what goes wrong.
pub(super) fn update(content: &str, chunks: &[Chunk], shown: &str) -> Result<String, String> {
    let rows: Vec<(&str, &str)> = content.split_inclusive('\n').map(ending).collect();
    let lines: Vec<&str> = rows.iter().map(|(text, _)| *text).collect();
    let preferred = rows.iter().map(|(_, end)| *end).find(|end| !end.is_empty());
    let preferred = preferred.unwrap_or("\n");
    let mut out = String::with_capacity(content.len());
    let mut next = 0;
    for edit in locate(&lines, chunks, shown)? {
        copy(&mut out, rows.get(next..edit.at), preferred);
        for text in edit.with {
            out.push_str(text);
            out.push_str(preferred);
        }
        next = next.max(edit.at + edit.len);
    }
    copy(&mut out, rows.get(next..), preferred);
    Ok(out)
}

/// `rows` as they were, a last line without an ending given the `preferred` one.
fn copy(out: &mut String, rows: Option<&[(&str, &str)]>, preferred: &str) {
    for (text, end) in rows.unwrap_or_default() {
        out.push_str(text);
        out.push_str(if end.is_empty() { preferred } else { end });
    }
}

/// The lines the chunks add and remove: those of context are in both and count for neither.
pub(super) fn counts(chunks: &[Chunk]) -> (usize, usize) {
    chunks.iter().fold((0, 0), |(added, removed), chunk| {
        (
            added + chunk.new.len().saturating_sub(chunk.kept.len()),
            removed + chunk.old.len().saturating_sub(chunk.kept.len()),
        )
    })
}

/// A row split into its text and its ending (none for a file's last line when it lacks one).
fn ending(row: &str) -> (&str, &str) {
    if let Some(text) = row.strip_suffix("\r\n") {
        (text, "\r\n")
    } else if let Some(text) = row.strip_suffix('\n') {
        (text, "\n")
    } else {
        (row, "")
    }
}

/// Where each chunk goes in `lines`, in file order.
fn locate<'a>(lines: &[&str], chunks: &'a [Chunk], shown: &str) -> Result<Vec<Edit<'a>>, String> {
    let mut edits = vec![];
    let mut from = 0;
    for chunk in chunks {
        let mut anchor = None;
        if let Some(context) = &chunk.context {
            let at = seek(lines, std::slice::from_ref(context), from, false)
                .ok_or_else(|| format!("could not find the line `{context}` in {shown}"))?;
            anchor = Some(at);
            // The hunk's first old line may be the anchor line itself.
            from = at;
        }
        if chunk.old.is_empty() {
            // Lines only added go below their anchor; with none, to the end.
            edits.push(Edit {
                at: anchor.map_or(lines.len(), |at| at + 1),
                len: 0,
                with: &chunk.new,
            });
            from = anchor.map_or(from, |at| at + 1);
            continue;
        }
        let (mut old, mut new) = (&chunk.old[..], &chunk.new[..]);
        let mut found = seek(lines, old, from, chunk.eof);
        // A blank last line stands for the file's final newline, which is not a line of its own.
        if found.is_none() && old.last().is_some_and(String::is_empty) {
            old = old.split_last().map_or(old, |(_, rest)| rest);
            if new.last().is_some_and(String::is_empty) {
                new = new.split_last().map_or(new, |(_, rest)| rest);
            }
            found = seek(lines, old, from, chunk.eof);
        }
        let Some(at) = found else {
            return Err(not_found(lines, &chunk.old, shown));
        };
        edits.extend(changes(chunk, old.len(), new, at));
        from = at + old.len();
    }
    edits.sort_by_key(|edit| edit.at);
    Ok(edits)
}

/// Why `old` was not found, with the file's lines nearest to it: copied from there, the next patch
/// needs no read first.
fn not_found(lines: &[&str], old: &[String], shown: &str) -> String {
    let wanted = old.join("\n");
    match nearest(lines, old) {
        Some(at) => {
            let near = lines.iter().skip(at).take(old.len());
            let near: Vec<&str> = near.copied().collect();
            format!(
                "could not find these lines in {shown}:\n{wanted}\nThe nearest lines in {shown}, \
                 from line {}:\n{}\nCopy the file's lines exactly, indentation included",
                at + 1,
                near.join("\n")
            )
        }
        None => format!(
            "could not find these lines in {shown}:\n{wanted}\nRead the file again and copy them \
             exactly, indentation included"
        ),
    }
}

/// The edits one chunk makes where its old lines were found at `at`: the lines of context between
/// the changes are left out, so they keep the file's own text.
fn changes<'a>(chunk: &Chunk, old_len: usize, new: &'a [String], at: usize) -> Vec<Edit<'a>> {
    let part = |from: usize, to: usize| new.get(from..to).unwrap_or_default();
    let (mut old_at, mut new_at) = (0, 0);
    let mut edits = vec![];
    for &(old_kept, new_kept) in &chunk.kept {
        if old_kept >= old_len || new_kept >= new.len() {
            break;
        }
        if (old_at, new_at) != (old_kept, new_kept) {
            edits.push(Edit {
                at: at + old_at,
                len: old_kept - old_at,
                with: part(new_at, new_kept),
            });
        }
        (old_at, new_at) = (old_kept + 1, new_kept + 1);
    }
    if (old_at, new_at) != (old_len, new.len()) {
        edits.push(Edit {
            at: at + old_at,
            len: old_len - old_at,
            with: part(new_at, new.len()),
        });
    }
    edits
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
