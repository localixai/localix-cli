//! `read`: a file's lines, numbered, a window at a time; or a directory's entries. The file is
//! streamed, so a huge log costs only the window shown (and a count of its lines).

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde::Deserialize;
use serde_json::{Value, json};

use super::text::{LINE_CHARS, cut};
use crate::workspace::Workspace;

const NAME: &str = "read";
/// Lines one call returns unless `limit` says otherwise.
const DEFAULT_LIMIT: usize = 2000;
/// Bytes of numbered lines one call returns at most, whatever `limit` says: below what the agent
/// keeps of one result, so the continuation hint is always true.
const MAX_BYTES: usize = 20_000;
/// Entries of a directory listed at most.
const MAX_ENTRIES: usize = 500;
/// Bytes at the start of a file searched for a NUL, as git does, to tell a binary file.
const BINARY_PROBE: usize = 8192;

const DESCRIPTION: &str = "Read a text file, or list a directory. A file comes back as numbered \
lines like `cat -n` (the number, a tab, the line); the numbers are not part of the file. One call \
returns up to 2000 lines from `offset` (1-based, default 1) and about 20 KB; for a longer file, \
call again with the `offset` it gives. Lines over 2000 characters are cut. Paths are relative to \
the workspace root; absolute paths work too. Binary files are refused. Read a file before editing \
it, and prefer this to `cat`, `head` or `sed` in bash.";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

pub struct ReadTool<'a> {
    pub workspace: &'a Workspace,
}

impl Tool for ReadTool<'_> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME.into(),
            description: DESCRIPTION.into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "The file or directory to read."},
                    "offset": {"type": "integer", "minimum": 1,
                        "description": "The first line to return, 1-based."},
                    "limit": {"type": "integer", "minimum": 1,
                        "description": "How many lines to return (default 2000)."},
                },
                "required": ["path"],
                "additionalProperties": false,
            }),
        }
    }

    /// Reading changes nothing: several files are read at once.
    fn parallel(&self) -> bool {
        true
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let args: Args = serde_json::from_value(arguments.clone())
            .map_err(|error| ToolError::Failed(format!("unusable arguments: {error}")))?;
        let path = self.workspace.readable(&args.path)?;
        let shown = self.workspace.shown(&path);
        let unreadable = |error: io::Error| failure(&shown, error);
        // Followed through symlinks. Anything but a file or a directory (a FIFO, a device such as
        // /dev/zero or /dev/tty) would block or never end, and no deadline covers a read.
        let meta = path.metadata().map_err(unreadable)?;
        let report = if meta.is_dir() {
            list(&path, &shown).map_err(unreadable)?
        } else if !meta.is_file() {
            return Err(ToolError::Failed(format!(
                "{shown} is not a regular file: it cannot be read here"
            )));
        } else {
            let first = args.offset.unwrap_or(1).max(1);
            let limit = args.limit.unwrap_or(DEFAULT_LIMIT).max(1);
            let file = File::open(&path).map_err(unreadable)?;
            lines(BufReader::new(file), &shown, first, limit)?
        };
        Ok(report.into())
    }
}

fn failure(shown: &str, error: io::Error) -> ToolError {
    ToolError::Failed(if error.kind() == io::ErrorKind::NotFound {
        format!("{shown} does not exist")
    } else {
        format!("{shown} could not be read: {error}")
    })
}

/// The directory's entries, sorted, each directory with a trailing `/`.
fn list(dir: &Path, shown: &str) -> io::Result<String> {
    let mut entries: Vec<String> = std::fs::read_dir(dir)?
        .flatten()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() {
                format!("{name}/")
            } else {
                name
            }
        })
        .collect();
    entries.sort();
    let total = entries.len();
    let head = format!(
        "{}/ ({total} {})",
        shown.trim_end_matches(['/', '\\']),
        if total == 1 { "entry" } else { "entries" }
    );
    let mut out = vec![head];
    out.extend(entries.into_iter().take(MAX_ENTRIES));
    if total > MAX_ENTRIES {
        out.push(format!(
            "... {} more not listed: use bash (`ls`, `find`, `rg --files`) to search it",
            total - MAX_ENTRIES
        ));
    }
    Ok(out.join("\n"))
}

/// Lines `first..` of `reader`, at most `limit` and [`MAX_BYTES`], under a header saying which.
fn lines(
    mut reader: impl BufRead,
    shown: &str,
    first: usize,
    limit: usize,
) -> Result<String, ToolError> {
    let mut line = Vec::new();
    let (mut total, mut probed, mut bytes) = (0, 0, 0);
    let mut out: Vec<String> = vec![];
    // Up to four bytes a character: enough of a line to show its first LINE_CHARS characters.
    while let Some(length) =
        next_line(&mut reader, &mut line, LINE_CHARS * 4).map_err(|e| failure(shown, e))?
    {
        if probed < BINARY_PROBE && line.contains(&0) {
            return Err(ToolError::Failed(format!(
                "{shown} is a binary file: it cannot be read as text"
            )));
        }
        probed += length + 1;
        total += 1;
        if total < first || out.len() == limit || bytes >= MAX_BYTES {
            continue;
        }
        let content = line.strip_suffix(b"\r").unwrap_or(&line);
        let whole = (length > line.len()).then_some(length);
        let numbered = format!(
            "{total:>6}\t{}",
            cut(&String::from_utf8_lossy(content), whole)
        );
        bytes += numbered.len() + 1;
        // The first line is always shown, so a call always makes progress.
        if bytes <= MAX_BYTES || out.is_empty() {
            out.push(numbered);
        }
    }
    if total == 0 {
        return Ok(format!("{shown} (empty file)"));
    }
    if first > total {
        return Err(ToolError::Failed(format!(
            "{shown} has {total} lines: offset {first} is past the end"
        )));
    }
    let last = first + out.len() - 1;
    let mut report = format!(
        "{shown} (lines {first}-{last} of {total})\n{}",
        out.join("\n")
    );
    if last < total {
        let next = last + 1;
        report.push_str(&format!(
            "\n\n(lines {next}-{total} not shown: read again with offset {next} to continue)"
        ));
    }
    Ok(report)
}

/// The next line of `reader` into `line` without its `\n`, keeping at most `keep` bytes of it (the
/// rest is read past, so one enormous line cannot fill memory); its whole length in bytes, or
/// `None` at the end.
fn next_line(
    reader: &mut impl BufRead,
    line: &mut Vec<u8>,
    keep: usize,
) -> io::Result<Option<usize>> {
    line.clear();
    let (mut length, mut started) = (0, false);
    loop {
        let buffer = match reader.fill_buf() {
            Ok(buffer) => buffer,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if buffer.is_empty() {
            return Ok(started.then_some(length));
        }
        started = true;
        let newline = buffer.iter().position(|&byte| byte == b'\n');
        let taken = newline.unwrap_or(buffer.len());
        let room = keep.saturating_sub(line.len()).min(taken);
        line.extend_from_slice(&buffer[..room]);
        length += taken;
        reader.consume(taken + usize::from(newline.is_some()));
        if newline.is_some() {
            return Ok(Some(length));
        }
    }
}

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
