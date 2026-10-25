//! Where the one daemon is found, and that there is one (docs/daemon.md): a socket per config
//! directory in the person's own runtime directory (a test home gets its own daemon), a lock on
//! the config directory a second daemon is refused, and a probe a client asks before it starts
//! one (`start`).

mod start;

use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use protocol::{Command, Done};

pub(crate) use self::start::{READY_LINE, reach, stop};

/// The credential store's directory: the lock is there, and its path names the socket.
pub(crate) fn config() -> Result<PathBuf, String> {
    provider_kit::store::dir(crate::compose::APP)
        .ok_or_else(|| "no home directory for the daemon's lock".into())
}

/// This binary's build: its version, and its size and time, so that a daemon an older binary
/// started is told apart. `LLX_BUILD` stands in for it (a test's older daemon).
pub(crate) fn build() -> String {
    if let Some(build) = std::env::var_os("LLX_BUILD") {
        return build.to_string_lossy().into_owned();
    }
    let file = std::env::current_exe().and_then(std::fs::metadata);
    let made = file.as_ref().ok().and_then(|file| file.modified().ok());
    let made = made.and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok());
    let size = file.map_or(0, |file| file.len());
    let made = made.map_or(0, |at| at.as_secs());
    format!("{}+{size:x}.{made:x}", env!("CARGO_PKG_VERSION"))
}

/// What a daemon says of itself.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Status {
    pub build: String,
    pub pid: u32,
    pub attached: usize,
    pub lanes: usize,
    pub working: usize,
    pub kept: usize,
    pub jobs: usize,
}

impl Status {
    /// Whether nothing needs the daemon: no terminal attached, no session open or moved to the
    /// background, no task working, no monitor or schedule running.
    pub(crate) fn idle(&self) -> bool {
        [
            self.attached,
            self.lanes,
            self.working,
            self.kept,
            self.jobs,
        ] == [0; 5]
    }
}

/// The daemon at `path`, if one answers within `wait`.
pub(crate) fn status(path: &Path, wait: Duration) -> Option<Status> {
    let client = client::Client::connect(path, |_| {}).ok()?;
    match client.call(Command::Status, wait) {
        Ok(Done::Status {
            build,
            pid,
            attached,
            lanes,
            working,
            kept,
            jobs,
        }) => Some(Status {
            build,
            pid,
            attached,
            lanes,
            working,
            kept,
            jobs,
        }),
        _ => None,
    }
}

/// `$TMPDIR`, else `/tmp`: where a user's sockets go.
fn runtime_dir() -> PathBuf {
    std::env::var_os("TMPDIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from)
}

/// Where the daemon for `config` listens, as a client finds it: the socket its lock names while
/// it holds the lock (it was started where `$TMPDIR` may be another than this process's, as by
/// an app that sets its own), else where this process would put one.
pub(crate) fn socket_path(config: &Path) -> PathBuf {
    let named = std::fs::read_to_string(config.join("daemon.lock")).ok();
    let named = named.and_then(|text| text.lines().nth(1).map(PathBuf::from));
    named
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| own_socket(config))
}

/// `$TMPDIR/llx-<uid>/<hash of the config directory>.sock`: short, as `sun_path` must be, and
/// the same for a config directory in every process and every build (FNV-1a, not std's hasher,
/// which may change). Where a daemon started by this process listens.
pub(crate) fn own_socket(config: &Path) -> PathBuf {
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

/// The daemon's lock on its config directory, held while it lives; let go when it drops. The
/// file names the daemon's process and its socket while it holds it.
pub(crate) struct Lock {
    held: File,
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.held.set_len(0);
    }
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
        let mut held = file;
        held.set_len(0)?;
        write!(
            held,
            "{}\n{}",
            std::process::id(),
            own_socket(config).display()
        )?;
        return Ok(Some(Lock { held }));
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
    client.call(Command::Work, wait).is_ok()
}

/// What the daemon writes after its ready line goes where its errors go: nobody reads its stdout.
pub(crate) fn quiet() {
    // SAFETY: dup2 takes two descriptors this process has open, and replaces only the first.
    unsafe { libc::dup2(2, 1) };
}

#[cfg(test)]
#[path = "daemon_tests.rs"]
mod tests;
