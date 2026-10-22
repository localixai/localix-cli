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

/// The file's text, trimmed; `None` when it is missing or empty.
pub fn read(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Write `text` to `path`, readable by the person alone.
pub fn save(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    // The mode above applies only to a new file: one saved before with wider permissions (by
    // hand, or under another umask) is narrowed before the secret goes in.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(text.as_bytes())
}

/// Remove `path`; whether there was one.
pub fn remove(path: &Path) -> std::io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
