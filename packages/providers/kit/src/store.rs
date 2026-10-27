//! Credential files: `<config>/<app slug>/<name>`, readable by the person alone (0600 on unix).
//! What they hold is never printed or logged.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::App;

/// `<config>/<app slug>`: `$XDG_CONFIG_HOME`, else `~/.config`, else `%APPDATA%`.
pub fn dir(app: App) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
    Some(base.join(app.slug))
}

/// `<config>/<app slug>/<name>`.
pub fn path(app: App, name: &str) -> Option<PathBuf> {
    dir(app).map(|dir| dir.join(name))
}

/// The file's text, trimmed; `None` when it is missing or empty. Trimmed in place: a file of
/// megabytes (the model catalog) is not held twice.
pub fn read(path: &Path) -> Option<String> {
    let mut text = std::fs::read_to_string(path).ok()?;
    text.truncate(text.trim_end().len());
    text.drain(..text.len() - text.trim_start().len());
    (!text.is_empty()).then_some(text)
}

/// Write `text` to `path`, readable by the person alone: whole or not at all, through a new file
/// renamed over the old one, so a save cut short (a process ending mid-write) leaves the old one.
pub fn save(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    // Its own name per process: two sessions saving at once do not write into one file.
    let temporary = path.with_extension(format!("tmp{}", std::process::id()));
    options.open(&temporary)?.write_all(text.as_bytes())?;
    std::fs::rename(&temporary, path)
}

/// Remove `path`; whether there was one.
pub fn remove(path: &Path) -> std::io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// `text` as a key, trimmed; `None` when it cannot be one (empty, or with spaces inside).
pub fn as_key(text: &str) -> Option<&str> {
    let key = text.trim();
    let bad = key.is_empty() || key.contains(|c: char| c.is_whitespace() || c.is_control());
    (!bad).then_some(key)
}

/// `key` as it may be shown: its first six and last four characters, or nothing of a short one.
#[must_use]
pub fn masked(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 16 {
        return "...".into();
    }
    let start: String = chars[..6].iter().collect();
    let end: String = chars[chars.len() - 4..].iter().collect();
    format!("{start}...{end}")
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
