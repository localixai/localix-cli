//! The person stopping a task: Ctrl-C (SIGINT) or SIGTERM during it, or closing the browser
//! window. The task ends the way the time limit ends it: the command running is killed with
//! everything it started (commands run in their own session, so the terminal's signal never
//! reaches them), and so is every terminal session `bash` keeps; the gate approves nothing more,
//! and the task ends at its next model turn as `interrupted` (exit 130), with its `run.completed`
//! event and its session saved. A second signal, or one while no task runs (at the prompt line),
//! quits at once, killing the terminal sessions first. A signal or the window stops every
//! session's task; the interface's keys stop one session's, by its id.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::output::lock;

/// What the signal handler and the browser window stop: every task running, in every session.
/// Only atomics, so a signal handler may use it.
pub(crate) struct Process {
    /// Raised by each stop: a task that began before it is stopped.
    generation: AtomicU64,
    /// How many tasks run.
    busy: AtomicUsize,
    /// Stopped while tasks ran: the next stop quits, until none runs.
    pending: AtomicBool,
}

/// The process's own: what its signal handler and the browser window stop.
pub(crate) static PROCESS: Process = Process::new();

impl Process {
    pub(crate) const fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            busy: AtomicUsize::new(0),
            pending: AtomicBool::new(false),
        }
    }

    /// Stop every task running. Returns whether to quit instead: none runs, or they were
    /// stopped already.
    pub(crate) fn stop(&self) -> bool {
        if self.busy.load(Ordering::SeqCst) == 0 {
            return true;
        }
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.pending.swap(true, Ordering::SeqCst)
    }

    /// Whether the tasks running were stopped.
    pub(crate) fn stopping(&self) -> bool {
        self.pending.load(Ordering::SeqCst)
    }
}

impl Default for Process {
    fn default() -> Self {
        Self::new()
    }
}

/// One session's task: whether it runs, and whether it was stopped, by itself or by the process.
pub(crate) struct Interrupt {
    process: &'static Process,
    busy: AtomicBool,
    stopped: AtomicBool,
    /// The process's generation when the task began.
    began: AtomicU64,
}

impl Interrupt {
    /// A session's, stopped by the process's signals too.
    pub(crate) fn new() -> Self {
        Self::of(&PROCESS)
    }

    pub(crate) fn of(process: &'static Process) -> Self {
        Self {
            process,
            busy: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            began: AtomicU64::new(process.generation.load(Ordering::SeqCst)),
        }
    }

    /// A task starts: nothing has stopped it yet, not even a stop of the process before it.
    pub(crate) fn begin(&self) {
        self.stopped.store(false, Ordering::SeqCst);
        let now = self.process.generation.load(Ordering::SeqCst);
        self.began.store(now, Ordering::SeqCst);
        if !self.busy.swap(true, Ordering::SeqCst) {
            self.process.busy.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// The task ended.
    pub(crate) fn end(&self) {
        if self.busy.swap(false, Ordering::SeqCst)
            && self.process.busy.fetch_sub(1, Ordering::SeqCst) == 1
        {
            self.process.pending.store(false, Ordering::SeqCst);
        }
    }

    /// Whether the task was stopped: by the person, or by a stop of the process since it began.
    pub(crate) fn stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
            || self.began.load(Ordering::SeqCst) != self.process.generation.load(Ordering::SeqCst)
    }

    /// Stop the task. Returns whether to quit instead: no task runs, or it was stopped already.
    pub(crate) fn stop(&self) -> bool {
        !self.busy.load(Ordering::SeqCst) || self.stopped.swap(true, Ordering::SeqCst)
    }
}

impl Default for Interrupt {
    fn default() -> Self {
        Self::new()
    }
}

type Stop = Arc<dyn Fn() -> bool + Send + Sync>;

/// The sessions', agents' and jobs' stops by id, for the person's keys in the interface, each
/// with the number its [`Known`] removes it by. Never used by the signal handler.
static STOPS: Mutex<Vec<(String, u64, Stop)>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(0);

/// A stop known by its id until this is dropped, when what it stops has ended: a stale id then
/// stops nothing.
#[must_use = "the stop is forgotten when this is dropped"]
pub(crate) struct Known(u64);

impl Drop for Known {
    fn drop(&mut self) {
        lock(&STOPS).retain(|(_, number, _)| *number != self.0);
    }
}

/// Know `stop` as the stop of session `id`: it returns whether to quit instead.
pub(crate) fn register(id: &str, stop: impl Fn() -> bool + Send + Sync + 'static) -> Known {
    let number = NEXT.fetch_add(1, Ordering::SeqCst);
    lock(&STOPS).push((id.to_string(), number, Arc::new(stop)));
    Known(number)
}

/// Know `flag` as the stop of agent or job `id`: a stop never quits for it.
pub(crate) fn register_flag(id: &str, flag: Arc<AtomicBool>) -> Known {
    register(id, move || {
        flag.store(true, Ordering::SeqCst);
        false
    })
}

/// What job `id` of lane `lane` is known by: each lane's jobs are numbered from one.
pub(crate) fn job_key(lane: &str, id: &str) -> String {
    format!("{lane}{JOB}{id}")
}

/// What sets a job's key apart: no session's or agent's id has it.
const JOB: char = '/';

/// The monitors and schedules running, in every lane of the process.
pub(crate) fn jobs() -> usize {
    let stops = lock(&STOPS);
    stops.iter().filter(|(id, _, _)| id.contains(JOB)).count()
}

/// Stop session `id`'s task: whether to quit instead (it was stopped already); an unknown one
/// is left alone.
pub(crate) fn stop(id: &str) -> bool {
    let found = lock(&STOPS)
        .iter()
        .rev()
        .find(|(known, _, _)| known == id)
        .map(|(_, _, stop)| Arc::clone(stop));
    let quit = found.is_some_and(|stop| stop());
    // A question the task waits on withdraws now, not at its next look.
    crate::server::ui::ring();
    quit
}

/// Tasks running in the process, in every session.
pub(crate) fn working() -> usize {
    PROCESS.busy.load(Ordering::SeqCst)
}

/// The browser window was closed: the tasks stop as on Ctrl-C, and the process never quits for
/// it (at the prompt line, the window opens again on the next task).
pub(crate) fn window_closed() {
    PROCESS.stop();
    crate::server::ui::ring();
}

/// The daemon's window was closed: it shows every terminal's tabs, and the person closed a
/// window, not a session, so nothing stops. A task working in it is told (`arrivals`, the
/// window's entry) and its browser calls fail; the window opens again when one is next used.
pub(crate) fn window_closed_served() {}

/// Quit now, as a second Ctrl-C does: the terminal sessions are killed first.
pub(crate) fn quit_now() -> ! {
    tools::shell::kill_all_now();
    // SAFETY: _exit ends the process at once; the sessions, the only thing that must not be left
    // behind, are killed.
    unsafe { libc::_exit(crate::exit::INTERRUPTED) }
}

/// Handle SIGINT and SIGTERM for the rest of the process.
#[cfg(unix)]
pub(crate) fn install() {
    extern "C" fn on_signal(_: libc::c_int) {
        const NOTE: &[u8] = b"\nllx: stopping the task (Ctrl-C again quits now)\n";
        if PROCESS.stop() {
            // Only atomics and kill: _exit skips the destructors that would close the sessions.
            tools::shell::kill_all_now();
            // SAFETY: _exit is async-signal-safe; nothing is left to flush that matters more than
            // quitting when asked twice.
            unsafe { libc::_exit(crate::exit::INTERRUPTED) };
        }
        // SAFETY: write is async-signal-safe, and NOTE outlives the call.
        unsafe { libc::write(2, NOTE.as_ptr().cast(), NOTE.len()) };
    }
    // SAFETY: a sigaction is integers, a mask and a handler's address: all zero is one, with
    // no handler, until it is set below.
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    // SA_RESTART keeps a model request or a read in progress from failing half-way: the task
    // stops at its next turn instead.
    action.sa_flags = libc::SA_RESTART;
    // SAFETY: sigemptyset writes only the mask, which lives here.
    unsafe { libc::sigemptyset(&mut action.sa_mask) };
    // The action is fully initialized before it is installed, and the handler does only what a
    // signal handler may (atomics, kill, write, _exit).
    for signal in [libc::SIGINT, libc::SIGTERM] {
        // SAFETY: as said above; the action outlives the call, which copies it.
        unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) };
    }
}

/// On Windows, Ctrl-C reaches the commands too (they share the console), and ends the process.
#[cfg(not(unix))]
pub fn install() {}

#[cfg(test)]
#[path = "interrupt_tests.rs"]
mod tests;
