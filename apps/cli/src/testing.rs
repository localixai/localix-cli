//! What the app's unit tests share: a fresh directory per test, a writer whose bytes the test
//! reads back, and fakes of the models (no network).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use contracts::chat::{Chat, ChatError, Completion, Request};
use contracts::model::{DecideError, Decider, Decision, Question};
use serde_json::Value;

/// A directory for one test, removed when it ends: tests run in parallel, so each gets its own.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let name = format!(
            "lxsurf-cli-{}-{}-{}",
            std::process::id(),
            session::now(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        // As the workspace names it (on Windows, without the verbatim `\\?\` prefix), so paths
        // compare equal with what the app reports.
        let root = tools::workspace::Workspace::new(&dir).unwrap();
        Self(root.root().to_path_buf())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Write `content` at `name` under the directory, creating its parents.
    pub fn file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A writer that keeps what is written, shared with the test that reads it.
#[derive(Clone, Default)]
pub struct Captured(Rc<RefCell<Vec<u8>>>);

impl Captured {
    pub fn text(&self) -> String {
        String::from_utf8(self.0.borrow().clone()).unwrap()
    }
}

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A writer whose reader has gone: every write is a broken pipe.
pub struct Closed;

impl Write for Closed {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
}

/// A chat model that answers from a script, then is unreachable.
pub struct ScriptedChat(RefCell<Vec<Completion>>);

impl ScriptedChat {
    pub fn new(replies: Vec<Completion>) -> Self {
        Self(RefCell::new(replies.into_iter().rev().collect()))
    }
}

impl Chat for ScriptedChat {
    fn complete(&self, _: Request) -> Result<Completion, ChatError> {
        self.0
            .borrow_mut()
            .pop()
            .ok_or_else(|| ChatError::Unavailable("the script ran out".into()))
    }
}

/// A decision model that is never there: the tests run without a browser.
pub struct NoDecider;

impl Decider for NoDecider {
    fn decide(&self, _: &Value, _: &BTreeMap<String, Question>) -> Result<Decision, DecideError> {
        Err(DecideError::Unavailable(
            "no decision model in tests".into(),
        ))
    }
}
