//! What the tools' tests share: a fresh directory per test, and a gate that records what it was
//! asked.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use contracts::gate::{Effect, Gate, Verdict};

/// A directory for one test, removed when the test ends: tests run in parallel, so each gets its
/// own, and no crate is needed for it.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let name = format!(
            "llx-tools-{}-{nanos}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// Write `content` at `name` under the directory, creating its parents.
    pub(crate) fn file(&self, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
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

/// A gate that records each effect as `tool: summary`, and denies with `deny` when it is set.
#[derive(Default)]
pub(crate) struct Recorder {
    pub asked: Mutex<Vec<String>>,
    pub deny: Option<String>,
}

impl Recorder {
    pub(crate) fn denying(reason: &str) -> Self {
        Self {
            deny: Some(reason.into()),
            ..Self::default()
        }
    }
}

impl Gate for Recorder {
    fn check(&self, effect: Effect) -> Verdict {
        self.asked
            .lock()
            .unwrap()
            .push(format!("{}: {}", effect.tool, effect.summary));
        match &self.deny {
            Some(reason) => Verdict::Deny(reason.clone()),
            None => Verdict::Allow,
        }
    }
}
