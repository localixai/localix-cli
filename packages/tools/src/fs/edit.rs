//! `edit`: a file changed by replacing exact text, or created whole. Exact replacement fails loudly
//! when the model's picture of the file is stale (the text is not there, or not only once), so the
//! model reads again instead of changing the wrong place.

use std::fs;
use std::io;
use std::path::Path;

use contracts::gate::Gate;
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde::Deserialize;
use serde_json::{Value, json};

use super::atomic;
use super::text::{changed, crlf, excerpt, has_line_numbers};
use crate::ask;
use crate::workspace::Workspace;

const NAME: &str = "edit";
/// Lines shown before and after a change.
const CONTEXT: usize = 3;

const DESCRIPTION: &str = "Change a file by replacing exact text. `old_string` must match the \
file exactly, whitespace and indentation included, and occur exactly once: add surrounding lines \
until it is unique, or set `replace_all` to change every occurrence (a rename). Copy it from a \
`read` result without the line-number prefix (the number and the tab). To create a file, or fill \
an empty one, pass an empty `old_string` and the whole content as `new_string`; a file with \
content is never overwritten that way. Read a file before editing it, and again when an edit \
fails. Files change only inside the workspace; missing directories are created and line endings \
are kept. Prefer this to `sed` or redirects in bash.";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

pub struct EditTool<'a> {
    pub workspace: &'a Workspace,
    pub gate: &'a dyn Gate,
}

impl Tool for EditTool<'_> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME.into(),
            description: DESCRIPTION.into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "The file to change or create."},
                    "old_string": {"type": "string",
                        "description": "The exact text to replace; empty to create the file."},
                    "new_string": {"type": "string", "description": "The text to put instead."},
                    "replace_all": {"type": "boolean",
                        "description": "Replace every occurrence, not just a unique one."},
                },
                "required": ["path", "old_string", "new_string"],
                "additionalProperties": false,
            }),
        }
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let args: Args = serde_json::from_value(arguments.clone())
            .map_err(|error| ToolError::Failed(format!("unusable arguments: {error}")))?;
        let path = self.workspace.writable(&args.path)?;
        let shown = self.workspace.shown(&path);
        if path.is_dir() {
            return Err(ToolError::Failed(format!(
                "{shown} is a directory: edit changes files"
            )));
        }
        // A FIFO or a device would block the read, or never end it.
        if fs::metadata(&path).is_ok_and(|meta| !meta.is_file()) {
            return Err(ToolError::Failed(format!(
                "{shown} is not a regular file: edit changes files"
            )));
        }
        let existing = contents(&path, &shown)?;
        let report = if args.old_string.is_empty() {
            self.create(&path, &shown, existing.as_deref(), &args.new_string)?
        } else {
            let Some(bytes) = existing else {
                return Err(ToolError::Failed(format!(
                    "{shown} does not exist: to create it, pass an empty old_string and the \
                     whole content as new_string"
                )));
            };
            let content = String::from_utf8(bytes).map_err(|_| {
                ToolError::Failed(format!(
                    "{shown} is not UTF-8 text: it cannot be edited here"
                ))
            })?;
            let change = replace(&content, &args, &shown)?;
            let summary = format!("edit {shown} (+{} -{} lines)", change.added, change.removed);
            ask(self.gate, NAME, &summary)?;
            unchanged(&path, &shown, Some(content.as_bytes()))?;
            write(&path, &shown, &change.content)?;
            change.report(&shown)
        };
        Ok(report.into())
    }
}

impl EditTool<'_> {
    /// `content` as a new file, or as the content of an empty one; never over a file with content.
    fn create(
        &self,
        path: &Path,
        shown: &str,
        existing: Option<&[u8]>,
        content: &str,
    ) -> Result<String, ToolError> {
        if existing.is_some_and(|bytes| !bytes.is_empty()) {
            return Err(ToolError::Failed(format!(
                "{shown} already exists and is not empty: read it and replace a part of it with \
                 old_string instead"
            )));
        }
        let lines = content.lines().count();
        ask(
            self.gate,
            NAME,
            &format!("create {shown} ({})", count(lines)),
        )?;
        unchanged(path, shown, existing)?;
        write(path, shown, content)?;
        Ok(format!("Created {shown} ({}).", count(lines)))
    }
}

/// The file's bytes, or `None` when there is no file yet.
fn contents(path: &Path, shown: &str) -> Result<Option<Vec<u8>>, ToolError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ToolError::Failed(format!(
            "{shown} could not be read: {error}"
        ))),
    }
}

/// Fail when the file is no longer what the change was made from: the approval can take minutes,
/// and an editor, a formatter or the person may change the file meanwhile. Their change is kept,
/// and the model reads the file again. An empty file counts as none, as `create` treats it.
fn unchanged(path: &Path, shown: &str, read: Option<&[u8]>) -> Result<(), ToolError> {
    let now = contents(path, shown)?;
    let empty = |bytes: Option<&[u8]>| bytes.is_none_or(<[u8]>::is_empty);
    if now.as_deref() == read || (empty(now.as_deref()) && empty(read)) {
        return Ok(());
    }
    Err(ToolError::Failed(format!(
        "{shown} changed while waiting for approval: read it again"
    )))
}

fn write(path: &Path, shown: &str, content: &str) -> Result<(), ToolError> {
    atomic::write(path, content.as_bytes())
        .map_err(|error| ToolError::Failed(format!("{shown} could not be written: {error}")))
}

/// What a replacement makes of a file.
#[derive(Debug)]
struct Change {
    /// The new content, with the file's own line endings.
    content: String,
    count: usize,
    added: usize,
    removed: usize,
    /// The lines around the first replacement, numbered as in the new content.
    excerpt: String,
}

impl Change {
    fn report(&self, shown: &str) -> String {
        let what = match self.count {
            1 => "replaced 1 occurrence".to_string(),
            n => format!("replaced {n} occurrences, the first shown"),
        };
        format!(
            "Edited {shown}: {what} (+{} -{} lines).\n{}",
            self.added, self.removed, self.excerpt
        )
    }
}

/// `args.old_string` replaced in `content`, once or (with `replace_all`) everywhere. A file whose
/// every line ends with `\r\n` is matched with `\n`, as the model writes, and keeps its `\r\n`.
fn replace(content: &str, args: &Args, shown: &str) -> Result<Change, ToolError> {
    let windows = crlf(content);
    let lf = |text: &str| {
        if windows {
            text.replace("\r\n", "\n")
        } else {
            text.to_string()
        }
    };
    let (text, old, new) = (lf(content), lf(&args.old_string), lf(&args.new_string));
    if old == new {
        return Err(ToolError::Failed(
            "old_string and new_string are the same: there is nothing to change".into(),
        ));
    }
    let count = text.matches(old.as_str()).count();
    if count == 0 {
        let mut message = format!(
            "old_string was not found in {shown}. Read the file again and copy the text exactly, \
             whitespace and indentation included"
        );
        if has_line_numbers(&old) {
            message.push_str(
                ": it seems to include the line numbers of a read; copy only what follows each tab",
            );
        } else if !windows && old.contains('\n') && text.contains("\r\n") {
            // read drops each line's `\r`, so a span across one cannot be copied from it.
            message.push_str(
                ": the file mixes \\r\\n and \\n line endings, which read does not show; edit \
                 one line at a time",
            );
        }
        return Err(ToolError::Failed(message));
    }
    if count > 1 && !args.replace_all {
        return Err(ToolError::Failed(format!(
            "old_string occurs {count} times in {shown}: add surrounding lines to make it unique, \
             or set replace_all to replace every one"
        )));
    }
    let edited = if args.replace_all {
        text.replace(old.as_str(), &new)
    } else {
        text.replacen(old.as_str(), &new, 1)
    };
    let at = text.find(old.as_str()).unwrap_or(0);
    let first = text[..at].matches('\n').count() + 1;
    let last = first + new.lines().count().max(1) - 1;
    let (added, removed) = changed(&old, &new);
    Ok(Change {
        excerpt: excerpt(&edited, first.saturating_sub(CONTEXT), last + CONTEXT),
        content: if windows {
            edited.replace('\n', "\r\n")
        } else {
            edited
        },
        count,
        added: added * count,
        removed: removed * count,
    })
}

/// `1 line`, `12 lines`.
fn count(lines: usize) -> String {
    match lines {
        1 => "1 line".into(),
        n => format!("{n} lines"),
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
