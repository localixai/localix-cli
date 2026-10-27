//! A client's way to its daemon (docs/daemon.md, "Lifecycle"): the one that answers, if it is of
//! this build; one of another build stopped when idle (`Status::idle`), kept while in use (this
//! session then runs in process); none, or the one stopped, replaced by `llx serve` started in a session of its own,
//! its ready line awaited.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
use std::time::Duration;

use super::{build, probe, socket_path, status};

/// How long a daemon has to say what it is.
const ASK: Duration = Duration::from_secs(2);
/// How long a daemon has to start, and to end when asked.
const READY: Duration = Duration::from_secs(10);
const ENDING: Duration = Duration::from_secs(10);
/// What `llx serve` says on stdout once clients may connect.
pub(crate) const READY_LINE: &str = "llx serve: ready";

/// The socket of this build's daemon for `config`, started when there is none; why not, when
/// it cannot be had.
pub(crate) fn reach(config: &Path) -> Result<PathBuf, String> {
    let path = socket_path(config);
    if let Some(found) = status(&path, ASK) {
        if found.build == build() {
            return Ok(path);
        }
        // One still in use is never stopped under a terminal, or under work kept for one.
        if !found.idle() {
            let (pid, other) = (found.pid, found.build);
            return Err(format!(
                "the daemon (pid {pid}) is build {other} and in use"
            ));
        }
        if !stop(&path) {
            return Err(format!("the daemon (pid {}) did not stop", found.pid));
        }
    }
    spawn(config)?;
    // The daemon that serves may have been started where `$TMPDIR` was another: its lock says.
    Ok(socket_path(config))
}

/// Ask the daemon at `path` to end, and wait until it has: its connection ends with it. Whether
/// none is left.
pub(crate) fn stop(path: &Path) -> bool {
    let (gone, ended) = sync_channel::<()>(1);
    let closed = Closed(gone);
    // What it sends is nothing to us; the connection's end is all we wait for.
    let Ok(client) = client::Client::connect(path, move |_| {
        let _ = &closed;
    }) else {
        return true;
    };
    // A daemon under load answers late: a stop is given as long as the daemon has to end. One
    // that ends before its answer leaves is gone, which is what was asked.
    match client.call(protocol::Command::Shutdown, ENDING) {
        Ok(_) | Err(client::CallError::Gone) => {}
        Err(client::CallError::Timeout | client::CallError::Failed(_)) => return false,
    }
    matches!(
        ended.recv_timeout(ENDING),
        Err(RecvTimeoutError::Disconnected)
    )
}

/// Dropped with the reader of a connection: the connection has ended.
struct Closed(#[expect(dead_code, reason = "held only to be dropped")] SyncSender<()>);

/// `llx serve` in a session of its own, with no terminal: neither this terminal's signals nor
/// its closing reach it. Its stdin is nothing, its errors go to `daemon.log` beside the lock.
fn spawn(config: &Path) -> Result<(), String> {
    let why = |error: std::io::Error| format!("the daemon could not start: {error}");
    std::fs::create_dir_all(config).map_err(why)?;
    let log = std::fs::File::options()
        .create(true)
        .append(true)
        .open(config.join("daemon.log"))
        .map_err(why)?;
    let mut command = Command::new(std::env::current_exe().map_err(why)?);
    // Which model a session uses is its client's to say (`server::place::words`), never what
    // the terminal that happened to start the daemon had set.
    for (name, _, _) in crate::server::place::MODEL_VARS {
        command.env_remove(name);
    }
    command
        .arg("serve")
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(log);
    let lead = || {
        // SAFETY: setsid takes nothing, is async-signal-safe, and changes only the child.
        unsafe { libc::setsid() };
        Ok(())
    };
    // SAFETY: between fork and exec the closure calls only setsid (above).
    unsafe { std::os::unix::process::CommandExt::pre_exec(&mut command, lead) };
    // The daemon outlives this client: a descriptor of the client's it inherited (one its host
    // gave it, one another thread was making) would stay open as long.
    tools::shell::streams_only(&mut command);
    let mut child = command.spawn().map_err(why)?;
    let out = child.stdout.take().ok_or("the daemon has no stdout")?;
    let (said, line) = sync_channel(1);
    let reader = std::thread::Builder::new().name("llx-daemon-ready".into());
    reader
        .spawn(move || {
            let mut first = String::new();
            let _ = BufReader::new(out).read_line(&mut first);
            let _ = said.send(first);
        })
        .map_err(why)?;
    match line.recv_timeout(READY) {
        Ok(first) if first.starts_with(READY_LINE) => Ok(()),
        // Another client's daemon took the lock first: that one serves.
        Ok(_) if probe(&socket_path(config), ASK) => Ok(()),
        Ok(_) => {
            let _ = child.wait();
            Err("the daemon did not start: see daemon.log in llx's config directory".into())
        }
        Err(_late) => {
            let _ = child.kill();
            Err("the daemon did not start in time".into())
        }
    }
}
