//! Every process of a session, killed by its session id. A command's shell leads a session of its
//! own, and one signal to its process group ends only that group: a shell with job control (an
//! interactive shell in a terminal, or `set -m`) puts each job in a group of its own, and a job
//! running in the background gets no hangup when the shell dies. Every process keeps the session
//! it was started in unless it calls `setsid` itself, so the session id finds them all. The
//! session's id is its leader's pid, which stays taken while the leader is not reaped, so the
//! sweep runs before the leader is reaped and never reaches a recycled pid.
//!
//! The processes are listed into a table on the stack, with system calls only (no allocation, no
//! lock), so the signal handler that quits at once may sweep too.

use std::thread;
use std::time::{Duration, Instant};

/// The most processes one pass looks at: the whole system's, on a table on the stack.
#[cfg(target_vendor = "apple")]
const MOST: usize = 8192;
/// The pause between passes, while a process that was signalled is still being torn down.
const PAUSE: Duration = Duration::from_millis(10);

/// What one pass found.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Pass {
    /// Processes of the session, its leader aside, sent SIGKILL.
    pub signalled: usize,
    /// Processes of the session that refused the signal (another user's, such as `sudo`).
    pub refused: usize,
}

/// Send SIGKILL to every process of session `sid` but its leader (the caller kills and reaps that
/// one). Only system calls, so a signal handler may call it.
pub(super) fn kill_members(sid: libc::pid_t) -> Pass {
    let mut pass = Pass::default();
    if sid <= 1 {
        return pass;
    }
    each_pid(|pid| {
        // SAFETY: getsid only reads which session a process is in.
        if pid == sid || unsafe { libc::getsid(pid) } != sid {
            return;
        }
        // SAFETY: kill only sends a signal; it touches nothing in this process.
        match unsafe { libc::kill(pid, libc::SIGKILL) } {
            0 => pass.signalled += 1,
            _ if std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) => {}
            _ => pass.refused += 1,
        }
    });
    pass
}

/// Kill every process of session `sid` but its leader, again until none is left (a process may
/// fork while it is killed), for at most `wait`; whether none is left.
#[expect(
    clippy::disallowed_methods,
    reason = "a backoff between kill passes: a process killed has no event to wait on, and this runs in a signal handler too"
)]
pub(super) fn end(sid: libc::pid_t, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        let pass = kill_members(sid);
        if pass.refused > 0 {
            return false;
        }
        if pass.signalled == 0 {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(PAUSE);
    }
}

/// Call `visit` with the pid of every process on the system.
#[cfg(target_vendor = "apple")]
fn each_pid(mut visit: impl FnMut(libc::pid_t)) {
    let mut pids = [0 as libc::pid_t; MOST];
    let size = libc::c_int::try_from(size_of_val(&pids)).unwrap_or(libc::c_int::MAX);
    // SAFETY: proc_listallpids writes at most `size` bytes into `pids`, which outlives the call.
    let count = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), size) };
    let count = usize::try_from(count).unwrap_or(0).min(MOST);
    pids[..count]
        .iter()
        .copied()
        .filter(|&pid| pid > 0)
        .for_each(&mut visit);
}

/// Call `visit` with the pid of every process on the system: the numbered entries of `/proc`,
/// read with `getdents64` into a buffer on the stack.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn each_pid(mut visit: impl FnMut(libc::pid_t)) {
    // An entry: inode (8 bytes), offset (8), its length (2), its type (1), then its name.
    const NAME: usize = 19;
    let flags = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC;
    // SAFETY: open reads a constant, NUL-terminated path.
    let dir = unsafe { libc::open(c"/proc".as_ptr(), flags) };
    if dir < 0 {
        return;
    }
    let mut buffer = [0u8; 16 * 1024];
    loop {
        // SAFETY: getdents64 writes at most `buffer.len()` bytes into `buffer`.
        let read =
            unsafe { libc::syscall(libc::SYS_getdents64, dir, buffer.as_mut_ptr(), buffer.len()) };
        let Ok(read) = usize::try_from(read) else {
            break;
        };
        if read == 0 {
            break;
        }
        let mut at = 0;
        while at + NAME < read {
            let length = usize::from(u16::from_ne_bytes([buffer[at + 16], buffer[at + 17]]));
            if length == 0 {
                break;
            }
            let end = (at + length).min(read);
            if let Some(pid) = number(&buffer[at + NAME..end]) {
                visit(pid);
            }
            at += length;
        }
    }
    // SAFETY: the descriptor was opened above and is closed once.
    unsafe { libc::close(dir) };
}

/// Elsewhere there is no list of processes to sweep: only the group's signal ends a session.
#[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
fn each_pid(_: impl FnMut(libc::pid_t)) {}

/// The pid a `/proc` entry's NUL-terminated name spells, or `None` for any other entry.
#[cfg_attr(
    not(any(target_os = "linux", target_os = "android", test)),
    expect(
        dead_code,
        reason = "used only where /proc lists a session's processes, and by its tests"
    )
)]
fn number(name: &[u8]) -> Option<libc::pid_t> {
    let digits = name.split(|&byte| byte == 0).next()?;
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    digits.iter().try_fold(0 as libc::pid_t, |pid, &digit| {
        pid.checked_mul(10)?
            .checked_add(libc::pid_t::from(digit - b'0'))
    })
}

#[cfg(test)]
#[path = "sweep_tests.rs"]
mod tests;
