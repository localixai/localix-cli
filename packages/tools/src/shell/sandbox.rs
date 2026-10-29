//! A command confined to reading (ARCHITECTURE.md B3): the operating system keeps it from changing
//! anything, whatever its text says. It reads every file but the app's own (on macOS); it writes only `/dev/null` and a
//! scratch directory of its own (its `TMPDIR`, removed when it ends); it opens no network or local
//! socket, so it reaches no other process to act for it. On macOS Seatbelt (`sandbox-exec`, a
//! profile closed by default), on Linux Landlock and a seccomp filter. Where neither can be set
//! up, the command is refused with the reason, never run open.

use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use contracts::gate::Verdict;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

/// What a command may do, as the gate answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Whatever the person running llx may.
    Open,
    /// Read, and change nothing.
    ReadOnly,
}

impl Scope {
    /// What a gate's verdict lets a command do; a refusal is its reason.
    pub fn of(verdict: Verdict) -> Result<Self, String> {
        match verdict {
            Verdict::Allow => Ok(Self::Open),
            Verdict::ReadOnly => Ok(Self::ReadOnly),
            Verdict::Deny(reason) => Err(reason),
        }
    }
}

/// What a confined command holds while it runs: its scratch directory, removed when this drops,
/// and on Linux the sandbox's rules, applied in the child before it starts.
#[derive(Debug, Default)]
pub struct Held {
    scratch: Option<PathBuf>,
    #[cfg(target_os = "linux")]
    rules: Option<std::os::fd::OwnedFd>,
}

impl Held {
    /// The command's scratch directory: its `TMPDIR`, set after the environment it runs with, so
    /// a client's own `TMPDIR` does not replace it.
    pub(super) fn scratch(&self) -> Option<&std::path::Path> {
        self.scratch.as_deref()
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(scratch) = &self.scratch {
            let _ = std::fs::remove_dir_all(scratch);
        }
    }
}

/// `sh -c command`, confined to reading: the command to start (a session's leader, as an open one
/// is) and what it holds until it ends.
#[cfg(target_os = "macos")]
pub(super) fn read_only(command: &str, kept: &[PathBuf]) -> io::Result<(Command, Held)> {
    let scratch = scratch()?;
    // The scratch is kept, to be removed, while the command it was made for runs.
    let held = Held {
        scratch: Some(scratch.clone()),
    };
    let shell = macos::shell(command, &scratch, kept)?;
    Ok((shell, held))
}

/// Landlock grants what it lets read and cannot take a path out of it: `kept` is left to the
/// workspace's own check.
#[cfg(target_os = "linux")]
pub(super) fn read_only(command: &str, _kept: &[PathBuf]) -> io::Result<(Command, Held)> {
    let scratch = scratch()?;
    // As on macOS; the rules' descriptor too, until the child has taken them on.
    let mut held = Held {
        scratch: Some(scratch.clone()),
        rules: None,
    };
    let rules = linux::rules(&scratch)?;
    let shell = linux::shell(command, &rules);
    held.rules = Some(rules);
    Ok((shell, held))
}

/// A new directory of the command's own under the system's temporary one, readable by the person
/// alone, as the sandbox names it: symlinks resolved (macOS's `/var` is `/private/var`).
fn scratch() -> io::Result<PathBuf> {
    static MADE: AtomicU64 = AtomicU64::new(0);
    let name = format!(
        "llx-read-only-{}-{}",
        std::process::id(),
        MADE.fetch_add(1, Ordering::Relaxed)
    );
    let dir = std::env::temp_dir().join(name);
    let mut builder = std::fs::DirBuilder::new();
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(&dir)?;
    std::fs::canonicalize(&dir)
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod tests;
