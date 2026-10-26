//! `@` in the composer: the workspace's files, found by the letters typed after it, so a file is
//! named by its path and the agent reads it. The list is read once, when `@` is first typed, on a
//! thread of its own: the interface draws on meanwhile, and the files show when it arrives.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Files listed at most: a larger tree is cut, not walked to its end.
const MOST: usize = 20_000;
/// Files offered at once.
const SHOWN: usize = 8;
/// How long git may take to list them (R10): past it, it is killed and the tree walked instead.
const GIT_WAIT: Duration = Duration::from_secs(3);

/// Where the list goes once read: to the interface's thread, as an input that wakes it.
pub(crate) type Arrive = Box<dyn FnOnce(Vec<String>) + Send>;

#[derive(Default)]
pub(crate) struct Files {
    root: Option<PathBuf>,
    /// Taken when the list is first wanted.
    arrive: Option<Arrive>,
    /// Each path read, with its lower case: made once, not at every key.
    list: Vec<(String, String)>,
}

impl Files {
    pub(crate) fn new(root: PathBuf, arrive: Arrive) -> Self {
        Self {
            root: Some(root),
            arrive: Some(arrive),
            list: vec![],
        }
    }

    /// The project the files are in.
    pub(crate) fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Starts reading the list the first time it is wanted, off the interface's thread; it comes
    /// back through the [`Arrive`] given, to be [`Files::loaded`].
    pub(crate) fn load(&mut self) {
        let (Some(root), Some(arrive)) = (self.root.clone(), self.arrive.take()) else {
            return;
        };
        let reader = thread::Builder::new().name("llx-files".into());
        // No thread to read it: no files are offered.
        let _ = reader.spawn(move || arrive(list(&root)));
    }

    /// The list, read.
    pub(crate) fn loaded(&mut self, list: Vec<String>) {
        let lowered = list.into_iter().map(|path| (path.to_lowercase(), path));
        self.list = lowered.collect();
    }

    /// The files whose paths hold `query`'s letters in order, the closest first.
    pub(crate) fn matching(&self, query: &str) -> Vec<String> {
        let query = query.to_lowercase();
        let mut found: Vec<(usize, &String)> = (self.list.iter())
            .filter_map(|(lower, path)| score(lower, &query).map(|s| (s, path)))
            .collect();
        let closer = |a: &(usize, &String), b: &(usize, &String)| {
            (a.0.cmp(&b.0))
                .then(a.1.len().cmp(&b.1.len()))
                .then(a.1.cmp(b.1))
        };
        // Only the closest are shown: they are picked first, and only they are sorted.
        if found.len() > SHOWN {
            found.select_nth_unstable_by(SHOWN, closer);
            found.truncate(SHOWN);
        }
        found.sort_by(closer);
        found.into_iter().map(|(_, path)| path.clone()).collect()
    }

    #[cfg(test)]
    pub(crate) fn with(list: Vec<String>) -> Self {
        let mut files = Self::default();
        files.loaded(list);
        files
    }
}

/// How well a path (`lower`, in lower case) matches `query` (lower is better): the file's name
/// holding it whole is best, then the path holding it whole, then its letters in order; `None`
/// when they are not there.
fn score(lower: &str, query: &str) -> Option<usize> {
    let name = lower.rsplit('/').next().unwrap_or(lower);
    if query.is_empty() {
        return Some(3);
    }
    if name.starts_with(query) {
        return Some(0);
    }
    if name.contains(query) {
        return Some(1);
    }
    if lower.contains(query) {
        return Some(2);
    }
    let mut letters = lower.chars();
    query.chars().all(|q| letters.any(|c| c == q)).then_some(4)
}

/// The workspace's files: what git tracks and would track, else a walk that skips hidden
/// folders and build output.
fn list(root: &Path) -> Vec<String> {
    let mut git = Command::new("git");
    git.args(["ls-files", "-co", "--exclude-standard"])
        .current_dir(root);
    if let Some(out) = within(git, GIT_WAIT) {
        let out = String::from_utf8_lossy(&out);
        return out.lines().take(MOST).map(str::to_string).collect();
    }
    let mut out = vec![];
    walk(root, root, &mut out);
    out
}

/// What `command` printed, if it ended well within `wait`; past it, it is killed (R10). Its
/// output is read on a thread of its own, so what is waited for is that output's end.
fn within(mut command: Command, wait: Duration) -> Option<Vec<u8>> {
    let mut child = (command.stdin(Stdio::null()))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let printed = child.stdout.take().and_then(|out| {
        let (read, done) = mpsc::sync_channel(1);
        let reader = thread::Builder::new().name("llx-files-out".into());
        let reading = reader.spawn(move || {
            let mut bytes = vec![];
            let _ = out.take(64 << 20).read_to_end(&mut bytes);
            let _ = read.send(bytes);
        });
        reading.ok().and_then(|_| done.recv_timeout(wait).ok())
    });
    let Some(printed) = printed else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    // Its output ended as it exited: the status is there to be read.
    let ended = child.wait().ok()?;
    ended.success().then_some(printed)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= MOST {
            return;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            walk(root, &path, out);
        } else if let Ok(relative) = path.strip_prefix(root) {
            out.push(relative.to_string_lossy().to_string());
        }
    }
}

#[cfg(test)]
#[path = "mention_tests.rs"]
mod tests;
