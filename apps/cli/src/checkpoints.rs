//! Checkpoints: where the conversation stood before each task, so `/rewind` (or Esc Esc on an
//! empty prompt) can take it back to before one of them. Files are not kept: the model changes
//! them through the shell, and nothing records a command's files (ARCHITECTURE.md B4).

use std::sync::Mutex;

/// One task: what it asked, and where the conversation stood before it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Task {
    pub(crate) goal: String,
    pub(crate) at: usize,
}

/// The tasks of the conversation, oldest first.
#[derive(Default)]
pub(crate) struct Checkpoints(Mutex<Vec<Task>>);

impl Checkpoints {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Task>> {
        crate::sync::lock(&self.0)
    }

    /// A task starts, the conversation `at` messages long.
    pub(crate) fn begin(&self, goal: &str, at: usize) {
        self.lock().push(Task {
            goal: goal.to_string(),
            at,
        });
    }

    pub(crate) fn tasks(&self) -> Vec<Task> {
        self.lock().clone()
    }

    /// Forget task `index` and those after it.
    pub(crate) fn forget(&self, index: usize) {
        self.lock().truncate(index);
    }
}

#[cfg(test)]
#[path = "checkpoints_tests.rs"]
mod tests;
