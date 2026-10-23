//! Where the one daemon is found, and that there is one (docs/daemon.md): a socket per config
//! directory in the person's own runtime directory (a test home gets its own daemon), a lock on
//! the config directory a second daemon is refused, and a probe a client asks before it starts
//! one.

use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `$TMPDIR`, else `/tmp`: where a user's sockets go.
fn runtime_dir() -> PathBuf {
    std::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from)
}

/// `$TMPDIR/llx-<uid>/<hash of the config directory>.sock`: short, as `sun_path` must be, and
/// the same for a config directory in every process and every build (FNV-1a, not std's hasher,
/// which may change).
pub(crate) fn socket_path(config: &Path) -> PathBuf {
    let hash = config
        .as_os_str()
        .as_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    runtime_dir()
        .join(format!("llx-{uid}"))
        .join(format!("{hash:016x}.sock"))
}

/// The daemon's lock on its config directory, held while it lives; let go when it drops.
pub(crate) struct Lock {
    _held: File,
}

/// The lock on `<config>/daemon.lock`; `None` when another daemon holds it.
pub(crate) fn lock(config: &Path) -> std::io::Result<Option<Lock>> {
    std::fs::create_dir_all(config)?;
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(config.join("daemon.lock"))?;
    // SAFETY: the descriptor is the open file's, alive for the call; flock touches nothing else.
    let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if locked == 0 {
        return Ok(Some(Lock { _held: file }));
    }
    let error = std::io::Error::last_os_error();
    match error.kind() {
        std::io::ErrorKind::WouldBlock => Ok(None),
        _ => Err(error),
    }
}

/// Whether a daemon answers at `path` within `wait`: a socket file with nobody behind it, or a
/// server that does not answer, is none.
pub(crate) fn probe(path: &Path, wait: Duration) -> bool {
    let Ok(client) = client::Client::connect(path, |_| {}) else {
        return false;
    };
    client.call(protocol::Command::Work, wait).is_ok()
}

#[cfg(test)]
#[path = "daemon_tests.rs"]
mod tests;
