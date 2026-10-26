//! Checkpoints: before a task's first change to a file, what the file
//! held (or that it did not exist), so `/rewind` (or Esc Esc on an empty prompt) can put the
//! files back as they were before any task of the conversation, the conversation too, or both.
//! Only `edit`'s changes are kept, the agents' among them; what a command changed is not.

use std::path::PathBuf;
use std::sync::Mutex;

use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::Value;
use tools::workspace::Workspace;

/// One task: what it asked, where the conversation stood before it, the files as it found them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Task {
    pub goal: String,
    pub at: usize,
    files: Vec<(PathBuf, Option<Vec<u8>>)>,
}

impl Task {
    /// How many files it changed.
    pub(crate) fn changed(&self) -> usize {
        self.files.len()
    }
}

/// The tasks of the conversation, oldest first.
#[derive(Default)]
pub(crate) struct Checkpoints(Mutex<Vec<Task>>);

impl Checkpoints {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Task>> {
        crate::output::lock(&self.0)
    }

    /// A task starts, the conversation `at` messages long.
    pub(crate) fn begin(&self, goal: &str, at: usize) {
        self.lock().push(Task {
            goal: goal.to_string(),
            at,
            files: vec![],
        });
    }

    pub(crate) fn tasks(&self) -> Vec<Task> {
        self.lock().clone()
    }

    /// Keep what `path` holds before the running task first changes it.
    fn keep(&self, path: PathBuf) {
        let mut tasks = self.lock();
        let Some(task) = tasks.last_mut() else {
            return;
        };
        if task.files.iter().all(|(kept, _)| *kept != path) {
            let before = std::fs::read(&path).ok();
            task.files.push((path, before));
        }
    }

    /// Put the files back as they were before task `index`, and forget it and those after it;
    /// the files written back, and any that could not be.
    pub(crate) fn restore(&self, index: usize) -> (usize, Vec<String>) {
        let mut tasks = self.lock();
        let mut restored = 0;
        let mut failed = vec![];
        // Newest first, so a file changed by several tasks ends as the oldest found it.
        for task in tasks.drain(index..).rev() {
            for (path, before) in task.files.into_iter().rev() {
                let done = match before {
                    Some(bytes) => std::fs::write(&path, bytes),
                    None => std::fs::remove_file(&path).or_else(|e| {
                        if e.kind() == std::io::ErrorKind::NotFound {
                            Ok(())
                        } else {
                            Err(e)
                        }
                    }),
                };
                match done {
                    Ok(()) => restored += 1,
                    Err(error) => failed.push(format!("{}: {error}", path.display())),
                }
            }
        }
        (restored, failed)
    }

    /// Forget task `index` and those after it, leaving the files as they are.
    pub(crate) fn forget(&self, index: usize) {
        self.lock().truncate(index);
    }
}

/// `edit`, keeping each file's content before its first change in the task.
pub(crate) struct Tracked<'a> {
    pub inner: &'a dyn Tool,
    pub workspace: &'a Workspace,
    pub checkpoints: &'a Checkpoints,
}

impl Tool for Tracked<'_> {
    fn spec(&self) -> ToolSpec {
        self.inner.spec()
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let path = arguments.get("path").and_then(Value::as_str);
        if let Some(path) = path.and_then(|p| self.workspace.writable(p).ok()) {
            self.checkpoints.keep(path);
        }
        self.inner.call(arguments)
    }
}

#[cfg(test)]
#[path = "checkpoints_tests.rs"]
mod tests;
