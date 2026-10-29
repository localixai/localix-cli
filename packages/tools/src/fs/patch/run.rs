//! A patch carried out: every hunk worked out first on the files as the hunks before it leave
//! them, the gate asked for each file it will change, and only then the files written. A hunk that
//! does not fit, a path outside the workspace or a refusal leaves every file as it was.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use contracts::gate::Gate;
use contracts::tool::{ToolError, ToolOutput};

use super::{Chunk, Hunk, apply, parse};
use crate::ask;
use crate::fs::{atomic, diff};
use crate::shell::Scope;
use crate::workspace::Workspace;

/// The tool the gate is told of.
const NAME: &str = "apply_patch";

/// A file as the patch sees it.
struct File {
    path: PathBuf,
    shown: String,
    /// What is on disk, until the patch writes it.
    disk: Option<Vec<u8>>,
    /// What the hunks so far leave: `None` when there is no such file.
    now: Option<Vec<u8>>,
}

/// One thing the patch does to one file: what the gate is told, and what the report says.
struct Step {
    subject: String,
    summary: String,
    done: String,
    diff: String,
}

pub(super) fn run(
    workspace: &Workspace,
    gate: &dyn Gate,
    patch: &str,
) -> Result<ToolOutput, ToolError> {
    // Whole or nothing: a failure always leaves the files as they were, and says so, so the model
    // sends the whole patch again rather than the part it thinks is left.
    carry_out(workspace, gate, patch).map_err(|error| match error {
        ToolError::Failed(why) => ToolError::Failed(format!(
            "{}. No file was changed.",
            why.trim_end().trim_end_matches('.')
        )),
        fatal @ ToolError::Fatal(_) => fatal,
    })
}

fn carry_out(workspace: &Workspace, gate: &dyn Gate, patch: &str) -> Result<ToolOutput, ToolError> {
    let hunks = parse(patch).map_err(ToolError::Failed)?;
    let (mut files, mut steps) = (vec![], vec![]);
    for hunk in hunks {
        plan(workspace, &mut files, &mut steps, hunk)?;
    }
    for step in &steps {
        // A read-only verdict runs a command in the sandbox; a file written cannot be (B5).
        if ask(gate, NAME, &step.subject, &step.summary)? == Scope::ReadOnly {
            return Err(ToolError::Failed(format!(
                "not allowed: {} is read-only here, and a patch changes files",
                step.subject
            )));
        }
    }
    let changed: Vec<&File> = files.iter().filter(|f| f.now != f.disk).collect();
    for file in &changed {
        unchanged(&file.path, &file.shown, file.disk.as_deref())?;
    }
    write(&changed)?;
    // The model reads which files, as Codex's `apply_patch` says them; the diff is for the person
    // alone (`ToolOutput::shown`).
    let marks: Vec<&str> = (steps.iter())
        .filter(|step| !step.done.is_empty())
        .map(|step| step.done.as_str())
        .collect();
    let shown: Vec<String> = (steps.iter())
        .filter(|step| !step.diff.is_empty())
        .map(|step| format!("Diff of {}:\n{}", step.subject, step.diff))
        .collect();
    Ok(ToolOutput {
        report: format!(
            "Success. Updated the following files:\n{}",
            marks.join("\n")
        ),
        shown: shown.join("\n\n"),
        ..ToolOutput::default()
    })
}

/// The writes of `changed` first, then its deletions, so a move that fails halfway keeps its
/// source. A failure puts back every file already written, from what was on disk.
fn write(changed: &[&File]) -> Result<(), ToolError> {
    let (written, removed): (Vec<&&File>, Vec<&&File>) =
        changed.iter().partition(|file| file.now.is_some());
    let mut done: Vec<&File> = vec![];
    for file in written.into_iter().chain(removed) {
        let result = match &file.now {
            Some(bytes) => atomic::write(&file.path, bytes),
            None => fs::remove_file(&file.path),
        };
        if let Err(error) = result {
            for back in done {
                let _ = match &back.disk {
                    Some(bytes) => atomic::write(&back.path, bytes),
                    None => fs::remove_file(&back.path),
                };
            }
            return Err(ToolError::Failed(format!(
                "{} could not be written: {error}",
                file.shown
            )));
        }
        done.push(file);
    }
    Ok(())
}

/// The file `name`, read from disk the first time the patch names it.
fn load<'a>(
    workspace: &Workspace,
    files: &'a mut Vec<File>,
    name: &str,
) -> Result<&'a mut File, ToolError> {
    let path = workspace.writable(name)?;
    let index = match files.iter().position(|file| file.path == path) {
        Some(index) => index,
        None => {
            let shown = workspace.shown(&path);
            if path.is_dir() {
                return Err(ToolError::Failed(format!(
                    "{shown} is a directory: {NAME} changes files"
                )));
            }
            // A FIFO or a device would block the read, or never end it.
            if fs::metadata(&path).is_ok_and(|meta| !meta.is_file()) {
                return Err(ToolError::Failed(format!(
                    "{shown} is not a regular file: {NAME} changes files"
                )));
            }
            let disk = contents(&path, &shown)?;
            files.push(File {
                path,
                shown,
                now: disk.clone(),
                disk,
            });
            files.len() - 1
        }
    };
    files
        .get_mut(index)
        .ok_or_else(|| ToolError::Failed("a file of the patch is lost".into()))
}

/// What `hunk` makes of the files, noted in `files`, and the steps it takes.
fn plan(
    workspace: &Workspace,
    files: &mut Vec<File>,
    steps: &mut Vec<Step>,
    hunk: Hunk,
) -> Result<(), ToolError> {
    match hunk {
        Hunk::Add { path, content } => {
            let file = load(workspace, files, &path)?;
            if file.now.as_ref().is_some_and(|bytes| !bytes.is_empty()) {
                return Err(ToolError::Failed(format!(
                    "{} already exists and is not empty: change it with '*** Update File'",
                    file.shown
                )));
            }
            let size = count(content.lines().count());
            steps.push(Step {
                summary: format!("create {} ({size})", file.shown),
                done: format!("A {}", file.shown),
                ..Step::of(&file.shown)
            });
            file.now = Some(content.into_bytes());
        }
        Hunk::Delete { path } => {
            let file = load(workspace, files, &path)?;
            let Some(bytes) = file.now.take() else {
                return Err(ToolError::Failed(format!("{} does not exist", file.shown)));
            };
            let size = count(String::from_utf8_lossy(&bytes).lines().count());
            steps.push(Step {
                summary: format!("delete {} ({size})", file.shown),
                done: format!("D {}", file.shown),
                ..Step::of(&file.shown)
            });
        }
        Hunk::Update { path, to, chunks } => {
            update(workspace, files, steps, &path, (to.as_deref(), &chunks))?;
        }
    }
    Ok(())
}

/// An update of `path` by `chunks`, and a move to `to` when there is one.
fn update(
    workspace: &Workspace,
    files: &mut Vec<File>,
    steps: &mut Vec<Step>,
    path: &str,
    (to, chunks): (Option<&str>, &[Chunk]),
) -> Result<(), ToolError> {
    let file = load(workspace, files, path)?;
    let (shown, source) = (file.shown.clone(), file.path.clone());
    let Some(bytes) = file.now.take() else {
        return Err(ToolError::Failed(format!("{shown} does not exist")));
    };
    let old = String::from_utf8(bytes).map_err(|error| {
        ToolError::Failed(format!(
            "{shown} is not UTF-8 text ({}): it cannot be edited here",
            error.utf8_error()
        ))
    })?;
    let new = apply::update(&old, chunks, &shown).map_err(ToolError::Failed)?;
    let diff = diff::unified(&old, &new);
    let moved = match to {
        Some(to) => move_to(workspace, files, to, &source, new)?,
        None => {
            file.now = Some(new.into_bytes());
            None
        }
    };
    let (added, removed) = apply::counts(chunks);
    let size = if chunks.is_empty() {
        String::new()
    } else {
        format!(" (+{added} -{removed} lines)")
    };
    let (done, summary) = match &moved {
        Some(dest) => (
            format!("D {shown}\nA {dest}"),
            format!("move {shown} to {dest}{size}"),
        ),
        None => (format!("M {shown}"), format!("update {shown}{size}")),
    };
    steps.push(Step {
        summary,
        done,
        diff,
        ..Step::of(&shown)
    });
    if let Some(dest) = moved {
        steps.push(Step {
            summary: format!("create {dest} (moved from {shown})"),
            ..Step::of(&dest)
        });
    }
    Ok(())
}

/// `new` put at `to` and the file at `source` (taken already) gone, when `to` is another, free
/// place; the name `to` is shown as, or none when the file stays where it is.
fn move_to(
    workspace: &Workspace,
    files: &mut Vec<File>,
    to: &str,
    source: &PathBuf,
    new: String,
) -> Result<Option<String>, ToolError> {
    let dest = load(workspace, files, to)?;
    if dest.path == *source {
        dest.now = Some(new.into_bytes());
        return Ok(None);
    }
    if dest.now.as_ref().is_some_and(|bytes| !bytes.is_empty()) {
        return Err(ToolError::Failed(format!(
            "{} already exists and is not empty: a move does not overwrite it",
            dest.shown
        )));
    }
    dest.now = Some(new.into_bytes());
    Ok(Some(dest.shown.clone()))
}

impl Step {
    /// A step on `shown`, its words still to say.
    fn of(shown: &str) -> Self {
        Self {
            subject: shown.to_string(),
            summary: String::new(),
            done: String::new(),
            diff: String::new(),
        }
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

/// Fail when the file is no longer what the patch was worked out from: the approval can take
/// minutes, and an editor, a formatter or the person may change the file meanwhile. Their change
/// is kept, and the model reads the file again.
fn unchanged(path: &Path, shown: &str, read: Option<&[u8]>) -> Result<(), ToolError> {
    if contents(path, shown)?.as_deref() == read {
        return Ok(());
    }
    Err(ToolError::Failed(format!(
        "{shown} changed while waiting for approval: read it again"
    )))
}

/// `1 line`, `12 lines`.
fn count(lines: usize) -> String {
    match lines {
        1 => "1 line".into(),
        n => format!("{n} lines"),
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
