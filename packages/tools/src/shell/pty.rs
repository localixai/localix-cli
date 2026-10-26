//! A command in a terminal of its own (a pseudo-terminal), for a program that must ask: a REPL, an
//! installer's questions. The shell leads a new session whose controlling terminal is the PTY, so
//! Ctrl-C reaches its foreground group as a terminal's would. A shell there has job control, so
//! its jobs get groups of their own: the session is ended by its id, not by the shell's group
//! (see `process::kill`). A thread reads the terminal into its [`Shared`] stream, and wakes
//! whoever waits on it.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::stream::Shared;
use super::{keys, process, watch};

/// The window's size, which never changes: wide enough that tables do not wrap.
pub(super) const COLUMNS: u16 = 120;
pub(super) const ROWS: u16 = 40;
/// Bytes written at a time, each when the terminal can take them.
const CHUNK: usize = 512;
/// How long a write may wait for a program that does not read its input.
const WRITE_WAIT: Duration = Duration::from_secs(2);
/// The pause before each key: text followed at once by Enter is taken for a paste.
const KEY_PAUSE: Duration = Duration::from_millis(30);
/// How often the reader looks whether the session was closed, while the terminal is silent.
const READ_POLL: Duration = Duration::from_millis(200);

pub(super) struct Pty {
    pub child: Child,
    writer: File,
    pub shared: Arc<Shared>,
}

/// Start `command` in `dir` in a new terminal with `env`, waiting for `needle` from its first
/// output on.
pub(super) fn spawn(
    command: &str,
    dir: &Path,
    env: process::Env,
    needle: Option<&str>,
) -> io::Result<Pty> {
    let (master, slave) = open()?;
    let mut shell = process::shell(command);
    process::quiet_env(&mut shell, env);
    // A real terminal: a full-screen program (an editor, an agent) draws as it
    // would anywhere, in colour for the window. What the model reads is text: a transcript, or
    // the screen when the program draws over it.
    shell
        .current_dir(dir)
        .env("TERM", "xterm-256color")
        .env("COLUMNS", COLUMNS.to_string())
        .env("LINES", ROWS.to_string())
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave.try_clone()?));
    let take_terminal = || {
        // A terminal's signals, whatever this process inherited: a shell starts a background job
        // (`llx exec … &`, `cargo test &`) with SIGINT and SIGQUIT ignored, an ignored signal
        // stays ignored across exec, and Ctrl-C would then do nothing.
        for signal in [libc::SIGINT, libc::SIGQUIT] {
            // SAFETY: signal is async-signal-safe; it resets one disposition of the forked child.
            unsafe { libc::signal(signal, libc::SIG_DFL) };
        }
        // SAFETY: ioctl is async-signal-safe and only reads its arguments: the PTY on stdin
        // becomes the terminal of the session `process::shell`'s setsid made, which has none.
        match unsafe { libc::ioctl(0, libc::TIOCSCTTY as _, 0) } {
            -1 => Err(io::Error::last_os_error()),
            _ => Ok(()),
        }
    };
    // SAFETY: between fork and exec the closure calls only async-signal-safe functions and
    // allocates nothing; it runs after `process::shell`'s setsid.
    unsafe { shell.pre_exec(take_terminal) };
    let child = shell.spawn();
    // Every copy of the terminal's other end is dropped here: while one is open in this process,
    // the reader would never see the end of the output.
    drop(shell);
    drop(slave);
    let mut child = child?;
    set_flag(
        master.as_raw_fd(),
        libc::F_GETFL,
        libc::F_SETFL,
        libc::O_NONBLOCK,
    )?;
    let writer = File::from(master);
    let reader = writer.try_clone()?;
    let shared = Arc::new(Shared::default());
    shared.lock().expect(needle, true);
    let stream = Arc::clone(&shared);
    let pid = child.id();
    let reading = thread::Builder::new().name("llx-terminal".into());
    if let Err(error) = reading.spawn(move || read(reader, &stream, pid)) {
        // No one would read it: the session ends before it is ever shown.
        process::kill(&mut child);
        return Err(error);
    }
    Ok(Pty {
        child,
        writer,
        shared,
    })
}

impl Pty {
    /// Type `input`, then press `keys` one at a time; the bytes sent, of all of them. A program
    /// that stops reading its input stops the typing.
    #[expect(
        clippy::disallowed_methods,
        reason = "a pause between keys, as a person's: text followed at once by Enter is taken for a paste"
    )]
    pub(super) fn type_in(&self, input: &[u8], keys: &[String]) -> (usize, usize) {
        let cursor = || self.shared.lock().application_cursor();
        let mut total = input.len();
        total += keys
            .iter()
            .map(|key| keys::encode(key, false).map_or(0, |bytes| bytes.len()))
            .sum::<usize>();
        let mut sent = put(&self.writer, input, Instant::now() + WRITE_WAIT);
        if sent < input.len() {
            return (sent, total);
        }
        for key in keys {
            thread::sleep(KEY_PAUSE);
            // The mode is read just before the key: the program may have changed it since.
            let bytes = keys::encode(key, cursor()).unwrap_or_default();
            let put = put(&self.writer, &bytes, Instant::now() + WRITE_WAIT);
            sent += put;
            if put < bytes.len() {
                break;
            }
        }
        (sent, total)
    }

    /// Whether the shell exited. It is left a zombie, not reaped: its process group stays its
    /// own until the group is killed, so the signal can never reach a recycled pid.
    pub(super) fn exited(&self) -> bool {
        // SAFETY: siginfo_t is a plain C struct, for which all zeros is a valid value.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let flags = libc::WEXITED | libc::WNOHANG | libc::WNOWAIT;
        let pid = libc::id_t::from(self.child.id());
        // SAFETY: waitid only writes into `info`, which lives through the call.
        match unsafe { libc::waitid(libc::P_PID, pid, &mut info, flags) } {
            0 => info.si_signo == libc::SIGCHLD,
            // Already reaped, or not waitable: it is not running.
            _ => true,
        }
    }

    /// Close the session's window at once, even one kept to say the session ended.
    pub(super) fn dismiss(&self) {
        let window = self.shared.lock().detach();
        self.shared.wake.notify_all();
        self.shared.tell(window);
    }

    /// The session ends: the reader stops at its next look.
    pub(super) fn shut(&self) {
        let window = self.shared.lock().close();
        self.shared.wake.notify_all();
        self.shared.tell(window);
    }
}

/// The terminal's two ends, closed on exec so no other command inherits them.
fn open() -> io::Result<(OwnedFd, OwnedFd)> {
    let (mut master, mut slave) = (-1, -1);
    let size = libc::winsize {
        ws_row: ROWS,
        ws_col: COLUMNS,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let (name, settings) = (std::ptr::null_mut(), std::ptr::null_mut::<libc::termios>());
    // SAFETY: openpty writes two descriptors and only reads `size`; all of them outlive the call.
    let opened = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            name,
            settings,
            (&raw const size).cast_mut(),
        )
    };
    if opened == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openpty succeeded, so it is an open descriptor that nothing else owns.
    let master = unsafe { OwnedFd::from_raw_fd(master) };
    // SAFETY: as above.
    let slave = unsafe { OwnedFd::from_raw_fd(slave) };
    for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
        set_flag(fd, libc::F_GETFD, libc::F_SETFD, libc::FD_CLOEXEC)?;
    }
    Ok((master, slave))
}

fn set_flag(fd: RawFd, get: libc::c_int, set: libc::c_int, flag: libc::c_int) -> io::Result<()> {
    // SAFETY: fcntl only reads the flags of a descriptor this process owns.
    let flags = unsafe { libc::fcntl(fd, get) };
    // SAFETY: fcntl only sets the flags of a descriptor this process owns.
    let done = unsafe { libc::fcntl(fd, set, flags | flag) };
    match done {
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(()),
    }
}

/// Whether `fd` is ready for `events` within `wait`.
fn ready(fd: RawFd, events: libc::c_short, wait: Duration) -> bool {
    let mut poll = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    let millis = libc::c_int::try_from(wait.as_millis()).map_or(libc::c_int::MAX, |ms| ms.max(1));
    // SAFETY: poll reads and writes one pollfd, which lives through the call.
    unsafe { libc::poll(&mut poll, 1, millis) > 0 }
}

/// Write `bytes` by [`CHUNK`]s as the terminal takes them, until `deadline`; the bytes written.
fn put(file: &File, bytes: &[u8], deadline: Instant) -> usize {
    let mut sent = 0;
    while sent < bytes.len() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() || !ready(file.as_raw_fd(), libc::POLLOUT, left) {
            break;
        }
        let end = (sent + CHUNK).min(bytes.len());
        match (&*file).write(&bytes[sent..end]) {
            Ok(0) => break,
            Ok(written) => sent += written,
            Err(error) if retry(&error) => {}
            Err(_) => break,
        }
    }
    sent
}

fn retry(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
    )
}

/// Read the terminal into `shared` until it closes (EOF on macOS, EIO on Linux once nothing holds
/// the other end) or the session ends, answering the program's queries on the way. While a
/// window waits for a paint, the reader wakes for it; at the end, the window says so.
fn read(file: File, shared: &Shared, pid: u32) {
    let mut buffer = [0; 8192];
    loop {
        let poll = {
            let stream = shared.lock();
            if stream.closed() {
                break;
            }
            stream.paint_wait().unwrap_or(READ_POLL)
        };
        if !ready(file.as_raw_fd(), libc::POLLIN, poll) {
            let paint = shared.lock().paint_due();
            shared.tell(paint);
            continue;
        }
        match (&file).read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                let (replies, paint) = {
                    let mut stream = shared.lock();
                    (stream.take_in(&buffer[..read]), stream.paint_due())
                };
                shared.wake.notify_all();
                put(&file, &replies, Instant::now() + WRITE_WAIT);
                shared.tell(paint);
            }
            Err(error) if retry(&error) => {}
            Err(_) => break,
        }
    }
    // How the program ended is read before the end is told: a call waiting on it reaps it then.
    let attached = shared.lock().attached();
    let status = attached.then(|| watch::status(pid)).flatten();
    // The end and the window's leaving happen under one lock: no window opens after it.
    let window = shared.lock().finish();
    shared.wake.notify_all();
    if let Some((painter, screen)) = window {
        watch::linger(shared, status, painter, &screen);
    }
}

#[cfg(test)]
#[path = "pty_tests.rs"]
mod tests;
