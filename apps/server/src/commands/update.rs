//! Checks for a new stable release in a terminal session, and installs it only on `--update`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use protocol::Event;
use provider_kit::http::{Reply, Web};

use crate::compose::APP;
use crate::error::{self, Failure};
use crate::exit;
use crate::server::Ui;

const URL: &str = "https://api.github.com/repos/localixai/localix-cli/releases/latest";
const SCRIPT: &str = include_str!("../../../../scripts/install.sh");
const MANAGED: &str = "this binary is not in the install script's directory; \
    update it the same way you installed it";

/// A strict numeric release tag; no release text other than this version is shown or passed on.
fn version(text: &str) -> Option<(u64, u64, u64)> {
    let mut pieces = text.split('.');
    let mut part = || {
        let piece = pieces.next()?;
        if piece.is_empty() || (piece.len() > 1 && piece.starts_with('0')) {
            return None;
        }
        piece
            .bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| piece.parse().ok())
            .flatten()
    };
    let triple = (part()?, part()?, part()?);
    pieces.next().is_none().then_some(triple)
}

fn newer(current: &str, reply: &Reply) -> Result<Option<String>, String> {
    if reply.status != 200 {
        return Err(format!("GitHub answered {}", reply.status));
    }
    let release: serde_json::Value =
        serde_json::from_str(&reply.body).map_err(|e| e.to_string())?;
    if release["draft"].as_bool() != Some(false) || release["prerelease"].as_bool() != Some(false) {
        return Err("GitHub did not return a stable release".into());
    }
    let tag = release["tag_name"]
        .as_str()
        .and_then(|tag| tag.strip_prefix('v'))
        .and_then(|tag| version(tag).map(|parsed| (tag, parsed)))
        .ok_or("GitHub returned an invalid release tag")?;
    let (current, pre) = current
        .split_once('-')
        .map_or((current, false), |(base, _)| (base, true));
    let current = version(current).ok_or("this build has an invalid version")?;
    Ok((tag.1 > current || (tag.1 == current && pre)).then(|| tag.0.to_string()))
}

fn latest() -> Result<Option<String>, String> {
    let reply = Web::new(APP, Duration::from_secs(5))
        .get(URL, &[("accept", "application/vnd.github+json")])
        .map_err(|e| e.to_string())?;
    newer(APP.version, &reply)
}

fn directory(exe: &Path, dir: &Path) -> Option<PathBuf> {
    let dir = dir.canonicalize().ok()?;
    let installed = dir.join("llx");
    // Never replace the binary behind a symlink managed by another installer.
    (installed == exe && installed.symlink_metadata().ok()?.is_file()).then_some(dir)
}

fn install_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let dir = std::env::var_os("LLX_INSTALL_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::home_dir().map(|home| home.join(".local/bin")))?;
    directory(&exe, &dir)
}

/// Run the same bundled checksum-verifying installer that a fresh install uses.
pub(super) fn install() -> Result<i32, Failure> {
    let dir = install_dir().ok_or_else(|| Failure::Environment(MANAGED.into()))?;
    let Some(tag) = latest()
        .map_err(|why| Failure::Environment(format!("could not check for updates: {why}")))?
    else {
        error::print(&format!("llx {} is up to date.\n", APP.version))?;
        return Ok(exit::OK);
    };
    error::note(&format!("Updating llx to {tag}…\n"));
    let mut command = tools::shell::shell(SCRIPT);
    command.env("LLX_VERSION", tag).env("LLX_INSTALL_DIR", dir);
    let output = tools::shell::query(command, Duration::from_secs(120)).map_err(|why| {
        Failure::Environment(format!(
            "installer {why}; see https://cli.localix.ai/docs/install"
        ))
    })?;
    error::print(&String::from_utf8_lossy(&output))?;
    Ok(exit::OK)
}

fn background(
    fetch: impl FnOnce() -> Result<Option<String>, String> + Send + 'static,
    notify: impl FnOnce(String) + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("llx-update-check".into())
        .spawn(move || {
            if let Ok(Some(tag)) = fetch() {
                notify(tag);
            }
        })
}

/// No task, prompt, or quit waits for GitHub. A failed check is silent.
pub(in crate::commands) fn check(ui: Ui) {
    if std::env::var("LLX_UPDATE_CHECK").as_deref() != Ok("0") {
        let _ = background(latest, move |tag| {
            let how = if install_dir().is_some() {
                "Run llx --update when ready."
            } else {
                "Update it the same way you installed it."
            };
            ui.send(Event::Note(format!("llx {tag} is available. {how}")));
        });
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
