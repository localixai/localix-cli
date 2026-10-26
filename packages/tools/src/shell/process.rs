//! One command run to its end, its deadline, or until it is stopped. On unix the shell starts a
//! session of its own: it has no controlling terminal, so a program that opens the terminal to
//! prompt (a password, a passphrase) fails at once instead of stopping in the background until the
//! deadline, and the session's processes, found by its id, are everything it started (see
//! `sweep`). Its output is read on threads, so a command that fills one pipe while the other is
//! read never stalls.

use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use super::output::Output;

/// How long the output may stay open after the shell exits: a background process it started may
/// hold the pipes, and is not waited for.
const DRAIN_GRACE: Duration = Duration::from_millis(500);
/// How long a killed process is given to be reaped.
const KILL_WAIT: Duration = Duration::from_secs(2);
/// The longest wait between looks at the command: a stop, and an exit that leaves the output open
/// to a background process, are noticed within it.
const POLL: Duration = Duration::from_millis(50);
/// The pipes read: stdout and stderr.
const PIPES: usize = 2;

/// The shell commands run with, as a person would type it.
#[cfg(unix)]
pub const SHELL: &str = "sh -c";
#[cfg(windows)]
pub const SHELL: &str = "cmd /C";

/// What to run, and the bounds it runs in.
pub(super) struct Job<'a> {
    pub command: &'a str,
    pub dir: &'a Path,
    pub timeout: Duration,
    pub env: Env<'a>,
    /// Whether the person stopped the task: the command is killed as soon as it says so.
    pub stop: &'a (dyn Fn() -> bool + Sync),
}

/// How a command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Exit(i32),
    /// Ended by a signal it did not handle; only unix has them.
    #[cfg_attr(not(unix), allow(dead_code, reason = "used only on unix"))]
    Signal(i32),
    /// Past its deadline; `killed` once it and everything it started are known to be gone.
    TimedOut {
        killed: bool,
    },
    /// Stopped by the person; `killed` as for a timeout.
    Stopped {
        killed: bool,
    },
    /// Neither a code nor a signal could be read.
    Unknown,
}

#[derive(Debug)]
pub(super) struct Finished {
    pub ending: Ending,
    pub elapsed: Duration,
    pub stdout: String,
    pub stderr: String,
    /// The output was still open when it was taken: a background process holds it.
    pub open: bool,
}

/// Run `job` in a shell, with no input.
pub(super) fn run(job: &Job) -> io::Result<Finished> {
    let mut shell = shell(job.command);
    quiet_env(&mut shell, job.env);
    let mut child = shell
        .current_dir(job.dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let started = Instant::now();
    // Two readers, one send each: room for both, so neither waits.
    let (closed, readers) = mpsc::sync_channel(PIPES);
    let stdout = drain(child.stdout.take(), closed.clone());
    let stderr = drain(child.stderr.take(), closed);
    let (ending, elapsed, ended) = watch(&mut child, job, started, &readers)?;
    let open = !drained(&readers, PIPES - ended);
    Ok(Finished {
        ending,
        elapsed,
        stdout: text(&stdout),
        stderr: text(&stderr),
        open,
    })
}

/// The environment a command runs with: the variables of the place it runs for (the client's,
/// not this process's), else this process's own; without `hidden`, the app's own secrets. A
/// command can still read those from its parent's environment (see `BashTool::hidden`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Env<'a> {
    pub vars: Option<&'a [(String, String)]>,
    pub hidden: &'a [&'a str],
}

impl Env<'_> {
    pub fn apply(self, command: &mut Command) {
        if let Some(vars) = self.vars {
            command
                .env_clear()
                .envs(vars.iter().map(|(name, value)| (name, value)));
        }
        for name in self.hidden {
            command.env_remove(name);
        }
    }
}

/// The environment every command runs in: `env`, with nothing that waits for an answer or a
/// pager, since there is no one to type or page.
pub(super) fn quiet_env(shell: &mut Command, env: Env) {
    env.apply(shell);
    shell
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat");
}

/// Wait for `child` to exit, killing it at its deadline or when the person stops the task; how it
/// ended, when, and how many of its pipes closed meanwhile. A command's exit closes its output, so
/// the wait is on its pipes' readers (`readers`, a message as each pipe closes): the exit is seen
/// at once (R9). The stop is looked at, and an exit that leaves the output to a background
/// process seen, within [`POLL`].
#[allow(
    clippy::disallowed_methods,
    reason = "a backoff of a few ms, bounded by the deadline: the pipes closed and the exit, which \
              std gives no event for, is a moment away"
)]
fn watch(
    child: &mut Child,
    job: &Job,
    started: Instant,
    readers: &Receiver<()>,
) -> io::Result<(Ending, Duration, usize)> {
    let deadline = started + job.timeout;
    let (mut ended, mut pause) = (0, Duration::from_millis(1));
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok((ending(status), started.elapsed(), ended)),
            Ok(None) => {}
            Err(error) => {
                kill(child);
                return Err(error);
            }
        }
        let late = Instant::now() >= deadline;
        if late || (job.stop)() {
            let elapsed = started.elapsed();
            let killed = kill(child);
            let ending = match late {
                true => Ending::TimedOut { killed },
                false => Ending::Stopped { killed },
            };
            return Ok((ending, elapsed, ended));
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if ended < PIPES {
            match readers.recv_timeout(POLL.min(left)) {
                Ok(()) => ended += 1,
                Err(RecvTimeoutError::Timeout) => {}
                // A reader gone without a word: no pipe will say more.
                Err(RecvTimeoutError::Disconnected) => ended = PIPES,
            }
        } else {
            thread::sleep(pause.min(left));
            pause = (pause * 2).min(POLL);
        }
    }
}

#[cfg(unix)]
pub(super) fn shell(command: &str) -> Command {
    use std::os::unix::process::CommandExt;
    let mut shell = Command::new("sh");
    shell.arg("-c").arg(command);
    // SAFETY: setsid takes nothing and only makes the forked child a session's leader.
    let lead = || match unsafe { libc::setsid() } {
        -1 => Err(io::Error::last_os_error()),
        _ => Ok(()),
    };
    // SAFETY: setsid is async-signal-safe, and nothing else runs between fork and exec. The new
    // session's process group has the shell's pid, which `kill` signals.
    unsafe { shell.pre_exec(lead) };
    super::inherit::streams_only(&mut shell);
    shell
}

#[cfg(windows)]
fn shell(command: &str) -> Command {
    use std::os::windows::process::CommandExt;
    // cmd parses its own command line: passed raw, the command reaches it as written.
    let mut shell = Command::new("cmd");
    shell.arg("/C").raw_arg(command);
    shell
}

/// Kill the command and everything it started: its process group, then every other process of its
/// session (the jobs a shell with job control put in groups of their own); whether all of it is
/// known to be gone. `child` must not be reaped yet: its pid is the session's id until it is.
#[cfg(unix)]
pub(super) fn kill(child: &mut Child) -> bool {
    // A pid the system gave: it is a pid_t, which `id` widened.
    let leader = child.id().cast_signed();
    // SAFETY: kill only sends a signal; it touches nothing in this process.
    let signalled = unsafe { libc::kill(-leader, libc::SIGKILL) } == 0;
    // No process left in the group: everything it started is gone already.
    let gone = !signalled && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
    if !signalled {
        let _ = child.kill();
    }
    // Before the reap, while the leader's pid still names the session.
    let swept = super::sweep::end(leader, KILL_WAIT);
    reaped(child) && (signalled || gone) && swept
}

/// Kill the command and everything it started (the process tree); whether all of it is known to
/// be gone.
#[allow(
    clippy::disallowed_methods,
    reason = "taskkill signals the tree and returns: it has no wait of its own"
)]
#[cfg(windows)]
fn kill(child: &mut Child) -> bool {
    let tree = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !tree {
        let _ = child.kill();
    }
    reaped(child) && tree
}

/// Whether `child` exited within [`KILL_WAIT`] of being killed.
#[allow(
    clippy::disallowed_methods,
    reason = "a child's exit has no event in std: it is looked at every 10 ms, for at most KILL_WAIT"
)]
fn reaped(child: &mut Child) -> bool {
    let deadline = Instant::now() + KILL_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => return false,
        }
    }
}

/// Read `pipe` to its end into an [`Output`] on a thread; say so on `closed` when it ends.
fn drain(pipe: Option<impl Read + Send + 'static>, closed: SyncSender<()>) -> Arc<Mutex<Output>> {
    let output = Arc::new(Mutex::new(Output::default()));
    let Some(mut pipe) = pipe else {
        let _ = closed.send(());
        return output;
    };
    let shared = Arc::clone(&output);
    let reader = thread::Builder::new().name("llx-pipe".into());
    let started = reader.spawn(move || {
        let mut buffer = [0; 8192];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => shared
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(&buffer[..read]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        let _ = closed.send(());
    });
    // No thread to read it: the pipe is dropped with the closure, the command sees it closed,
    // and its output is told as empty.
    drop(started);
    output
}

/// Whether `count` readers closed within [`DRAIN_GRACE`].
fn drained(readers: &Receiver<()>, count: usize) -> bool {
    let deadline = Instant::now() + DRAIN_GRACE;
    (0..count).all(|_| {
        readers
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .is_ok()
    })
}

fn text(output: &Mutex<Output>) -> String {
    output.lock().unwrap_or_else(PoisonError::into_inner).text()
}

pub(super) fn ending(status: ExitStatus) -> Ending {
    if let Some(code) = status.code() {
        return Ending::Exit(code);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return Ending::Signal(signal);
        }
    }
    Ending::Unknown
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
