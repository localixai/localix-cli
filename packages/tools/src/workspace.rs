//! The directory the tools work in: where a command runs, and where the `workdir` the model writes
//! resolves; a directory the app keeps to itself (its keys, its records) is never one.

use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use contracts::tool::ToolError;

#[derive(Debug, Clone)]
pub struct Workspace {
    /// Canonical, so a path is compared with it after symlinks are resolved.
    root: PathBuf,
    /// Files and directories no tool may read or change, however they are reached (the app's own
    /// key and records): canonical where they exist.
    denied: Vec<PathBuf>,
    /// The environment commands run with, when it is not this process's: the client's.
    env: Option<Arc<[(String, String)]>>,
}

impl Workspace {
    /// The workspace at `root`, which must exist.
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            root: canonical(root.as_ref())?,
            denied: vec![],
            env: None,
        })
    }

    /// The same workspace, with `paths` (and everything under them) off limits to the tools.
    #[must_use]
    pub fn denying(mut self, paths: impl IntoIterator<Item = PathBuf>) -> Self {
        self.denied
            .extend(paths.into_iter().map(|path| real(&path)));
        self
    }

    /// The same workspace, its commands run with `env` instead of this process's environment.
    #[must_use]
    pub fn with_env(mut self, env: Vec<(String, String)>) -> Self {
        self.env = Some(env.into());
        self
    }

    pub fn env(&self) -> Option<&[(String, String)]> {
        self.env.as_deref()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// What the app keeps to itself, canonical where it exists: a read-only command's sandbox
    /// leaves it unread where the system can.
    pub fn denied(&self) -> &[PathBuf] {
        &self.denied
    }

    /// Where `path` points, for reading: relative to the root, or absolute; `~` is the home
    /// directory. `.` and `..` are resolved by name, so the result is clean in reports.
    pub fn readable(&self, path: &str) -> Result<PathBuf, ToolError> {
        let path = path.trim();
        if path.is_empty() {
            return Err(ToolError::Failed("the path is empty".into()));
        }
        let given = home(path).unwrap_or_else(|| PathBuf::from(path));
        let wanted = normal(&self.root.join(given));
        if !self.denied.is_empty() {
            // Where it really is: a symlink to a denied file is denied too.
            let real = real(&wanted);
            if self.denied.iter().any(|denied| real.starts_with(denied)) {
                return Err(ToolError::Failed(format!(
                    "{} is off limits to the tools",
                    wanted.display()
                )));
            }
        }
        Ok(wanted)
    }

    /// `path` as a report shows it: relative to the root when inside it, else as it is.
    pub fn shown(&self, path: &Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(inside) if inside.as_os_str().is_empty() => ".".into(),
            Ok(inside) => inside.display().to_string(),
            Err(_) => path.display().to_string(),
        }
    }
}

/// `~` or `~/rest` in the home directory, when it is known.
fn home(path: &str) -> Option<PathBuf> {
    let rest = path.strip_prefix('~')?;
    if !(rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\')) {
        return None;
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(rest.trim_start_matches(['/', '\\'])))
}

/// `path` with `.` dropped and each `..` taking off the name before it, without touching the disk.
fn normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other @ (Component::Prefix(_) | Component::RootDir | Component::Normal(_)) => {
                out.push(other)
            }
        }
    }
    out
}

/// `path` with its deepest existing ancestor canonicalized: symlinks on the way are followed, and
/// the names that do not exist yet (a file to create) are kept as they are.
pub(crate) fn real(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut missing = vec![];
    loop {
        if let Ok(canonical) = canonical(&existing) {
            return missing
                .iter()
                .rev()
                .fold(canonical, |acc: PathBuf, name| acc.join(name));
        }
        match (existing.file_name(), existing.parent()) {
            (Some(name), Some(parent)) => {
                missing.push(name.to_os_string());
                existing = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// `path` canonicalized, as a person writes it. On Windows, `canonicalize` returns a verbatim path
/// (`\\?\C:\...`), which cmd.exe cannot use as its directory (it falls back to C:\Windows) and
/// which reads oddly in every report; it is turned back into the plain form when that means the
/// same file, as the `dunce` crate does.
fn canonical(path: &Path) -> io::Result<PathBuf> {
    path.canonicalize().map(|path| plain(&path))
}

#[cfg(not(windows))]
fn plain(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(windows)]
fn plain(path: &Path) -> PathBuf {
    path.to_str()
        .and_then(unverbatim)
        .map_or_else(|| path.to_path_buf(), PathBuf::from)
}

/// `\\?\C:\rest` as `C:\rest`, and `\\?\UNC\server\share` as `\\server\share`, when the plain
/// form names the same file: shorter than MAX_PATH, and with no name Windows reads otherwise (a
/// device such as `NUL`, a trailing dot or space). `None` keeps the verbatim form.
#[cfg_attr(
    not(any(windows, test)),
    expect(dead_code, reason = "used only on Windows, and by its tests")
)]
fn unverbatim(path: &str) -> Option<String> {
    const MAX_PATH: usize = 260;
    let rest = path.strip_prefix(r"\\?\")?;
    let (plain, names) = match rest.strip_prefix(r"UNC\") {
        Some(share) => (format!(r"\\{share}"), share),
        None => {
            let drive = rest.as_bytes();
            let lettered = drive.len() >= 3
                && drive[0].is_ascii_alphabetic()
                && drive[1] == b':'
                && drive[2] == b'\\';
            if !lettered {
                return None;
            }
            (rest.to_string(), rest.get(3..).unwrap_or_default())
        }
    };
    let ordinary = names
        .split('\\')
        .filter(|name| !name.is_empty())
        .all(ordinary);
    (plain.len() < MAX_PATH && ordinary).then_some(plain)
}

/// A file name that means itself outside a verbatim path.
fn ordinary(name: &str) -> bool {
    if name.ends_with(['.', ' ']) || name.contains(['/', ':', '*', '?', '"', '<', '>', '|']) {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    let stem = stem.to_ascii_uppercase();
    let numbered = stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.as_bytes()[3].is_ascii_digit();
    !numbered
        && !matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        )
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
