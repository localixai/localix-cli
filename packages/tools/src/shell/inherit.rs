//! What a command inherits of this process: its standard streams, and no other descriptor.
//!
//! Everything this process opens is meant to close on exec, but some descriptors are made in two
//! steps, opened and then marked: on macOS std's pipes and sockets (it has no `pipe2`), and the
//! terminal's two ends from `openpty` everywhere. A command another thread forks between the two
//! steps inherits the descriptor, and a background process it starts keeps it as long as it
//! lives: another command's output pipe, which then never ends, or the pipe std's `spawn` reads to
//! learn whether exec succeeded, which keeps that `spawn` waiting until the background process
//! dies. A descriptor this process itself inherited open would reach every command the same way.
//!
//! So between fork and exec every descriptor above stderr is marked close-on-exec. The standard
//! streams are in place by then (std moves them before it runs the hooks), and std's own pipe for
//! exec's error is close-on-exec already: it stays open until exec, which closes it as it should.

use std::os::unix::process::CommandExt;
use std::process::Command;

/// Let `command` inherit only its standard streams. A command with this hook starts by fork and
/// exec, never by `posix_spawn`.
pub fn streams_only(command: &mut Command) {
    let hook = || {
        close_on_exec();
        Ok(())
    };
    // SAFETY: between fork and exec the hook makes only system calls that are async-signal-safe
    // (`close_on_exec`'s), takes no lock and allocates nothing.
    unsafe { command.pre_exec(hook) };
}

/// Mark every descriptor above stderr close-on-exec; a number that is not open is passed over.
#[cfg(target_vendor = "apple")]
fn close_on_exec() {
    /// Where the descriptor table's size cannot be read: `OPEN_MAX`, the most descriptors a
    /// process opens unless its limit was raised past it.
    const OPEN_MAX: libc::c_int = 10_240;
    // SAFETY: getpid takes nothing and cannot fail.
    let pid = unsafe { libc::getpid() };
    // Asked with no buffer, the bytes a list of every open descriptor would take: an entry for
    // each slot of the table (every open descriptor is below their count) and 20 more. One
    // system call on this process's own table: the child's, which no other thread changes.
    // SAFETY: with a null buffer of no length, proc_pidinfo writes nothing.
    let listed =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
    let end = match listed {
        bytes if bytes > 0 => bytes / libc::PROC_PIDLISTFD_SIZE,
        _ => OPEN_MAX,
    };
    for fd in 3..end {
        // SAFETY: fcntl sets one descriptor flag of the forked child, or fails on a number that
        // is not open.
        unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
    }
}

/// Mark every descriptor above stderr close-on-exec, in one system call (Linux 5.11 on). An older
/// kernel refuses it, and the command inherits what it did before; std makes its own descriptors
/// close-on-exec in one step there, so only the terminal's two ends were ever at risk.
#[cfg(target_os = "linux")]
fn close_on_exec() {
    let (first, last): (libc::c_uint, libc::c_uint) = (3, libc::c_uint::MAX);
    // SAFETY: close_range only marks descriptors of the forked child close-on-exec.
    unsafe {
        libc::syscall(
            libc::SYS_close_range,
            first,
            last,
            libc::CLOSE_RANGE_CLOEXEC,
        )
    };
}

/// Elsewhere commands inherit what they always did.
#[cfg(not(any(target_vendor = "apple", target_os = "linux")))]
fn close_on_exec() {}
