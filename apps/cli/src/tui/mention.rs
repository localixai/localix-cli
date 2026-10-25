//! `@` in the composer: the workspace's files, found by the letters typed after it, so a file is
//! named by its path and the agent reads it. The list is read once, when `@` is first typed.

use std::path::{Path, PathBuf};

/// Files listed at most: a larger tree is cut, not walked to its end.
const MOST: usize = 20_000;
/// Files offered at once.
const SHOWN: usize = 8;

#[derive(Default)]
pub(crate) struct Files {
    root: Option<PathBuf>,
    list: Option<Vec<String>>,
}

impl Files {
    pub(crate) fn new(root: Option<PathBuf>) -> Self {
        Self { root, list: None }
    }

    /// The project the files are in.
    pub(crate) fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Reads the list the first time it is wanted.
    pub(crate) fn load(&mut self) {
        if self.list.is_none() {
            self.list = Some(self.root.as_deref().map(list).unwrap_or_default());
        }
    }

    /// The files whose paths hold `query`'s letters in order, the closest first.
    pub(crate) fn matching(&self, query: &str) -> Vec<String> {
        let Some(list) = &self.list else {
            return vec![];
        };
        let query = query.to_lowercase();
        let mut found: Vec<(usize, &String)> = list
            .iter()
            .filter_map(|path| score(path, &query).map(|s| (s, path)))
            .collect();
        found.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.len().cmp(&b.1.len()))
                .then(a.1.cmp(b.1))
        });
        found
            .into_iter()
            .take(SHOWN)
            .map(|(_, path)| path.clone())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn with(list: Vec<String>) -> Self {
        Self {
            root: None,
            list: Some(list),
        }
    }
}

/// How well `path` matches `query` (lower is better): the file's name holding it whole is best,
/// then the path holding it whole, then its letters in order; `None` when they are not there.
fn score(path: &str, query: &str) -> Option<usize> {
    let lower = path.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
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
#[allow(
    clippy::disallowed_methods,
    reason = "R10 debt: a local git query, with no deadline or stop yet"
)]
fn list(root: &Path) -> Vec<String> {
    let git = std::process::Command::new("git")
        .args(["ls-files", "-co", "--exclude-standard"])
        .current_dir(root)
        .stderr(std::process::Stdio::null())
        .output();
    if let Ok(out) = git.and_then(|out| match out.status.success() {
        true => Ok(out),
        false => Err(std::io::Error::other("not a git work tree")),
    }) {
        return String::from_utf8_lossy(&out.stdout)
            .lines()
            .take(MOST)
            .map(str::to_string)
            .collect();
    }
    let mut out = vec![];
    walk(root, root, &mut out);
    out
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
