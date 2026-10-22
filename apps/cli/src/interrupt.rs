//! The person stopping a task: Ctrl-C (SIGINT) or SIGTERM during it, or closing the browser
//! window. The task ends the way the time limit ends it: the command running is killed with
//! everything it started (commands run in their own session, so the terminal's signal never
//! reaches them), and so is every terminal session `bash` keeps; the gate approves nothing more,
//! and the task ends at its next model turn as `interrupted` (exit 130), with its `run.completed`
//! event and its session saved. A second signal, or one while no task runs (at the prompt line),
//! quits at once, killing the terminal sessions first.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether a task runs, and whether the person stopped it.
pub struct Interrupt {
    busy: AtomicBool,
    stopped: AtomicBool,
}

/// The process's own: what its signal handler and the browser window set.
pub static PROCESS: Interrupt = Interrupt::new();

impl Interrupt {
    pub const fn new() -> Self {
        Self {
            busy: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
        }
    }

    /// A task starts: nothing has stopped it yet.
    pub fn begin(&self) {
        self.stopped.store(false, Ordering::SeqCst);
        self.busy.store(true, Ordering::SeqCst);
    }

    /// The task ended.
    pub fn end(&self) {
        self.busy.store(false, Ordering::SeqCst);
    }

    /// Whether the person stopped the task.
    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    /// Stop the task. Returns whether it should quit instead: no task runs, or it was stopped
    /// already. Only atomics, so a signal handler may call it.
    pub fn stop(&self) -> bool {
        !self.busy.load(Ordering::SeqCst) || self.stopped.swap(true, Ordering::SeqCst)
    }
}

impl Default for Interrupt {
    fn default() -> Self {
        Self::new()
    }
}

/// The browser window was closed: the task stops as on Ctrl-C, and the process never quits for it
/// (at the prompt line, the window opens again on the next task).
pub fn window_closed() {
    PROCESS.stop();
}

/// Handle SIGINT and SIGTERM for the rest of the process.
#[cfg(unix)]
pub fn install() {
    extern "C" fn on_signal(_: libc::c_int) {
        const NOTE: &[u8] = b"\nlx: stopping the task (Ctrl-C again quits now)\n";
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
    // SAFETY: the action is fully initialized before it is installed, and the handler does only
    // what a signal handler may (atomics, kill, write, _exit). SA_RESTART keeps a model request or a
    // read in progress from failing half-way: the task stops at its next turn instead.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        action.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut action.sa_mask);
        libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut());
        libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut());
    }
}

/// On Windows, Ctrl-C reaches the commands too (they share the console), and ends the process.
#[cfg(not(unix))]
pub fn install() {}

#[cfg(test)]
#[path = "interrupt_tests.rs"]
mod tests;
