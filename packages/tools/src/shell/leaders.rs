//! The sessions the shell tools have running, so a process that must end at once (a second Ctrl-C,
//! a hangup) can end them first: a command runs in a session of its own, which the terminal's
//! signals never reach and which outlives its parent. The ids are the leaders' pids, which stay
//! taken while a leader is not reaped (see `sweep`).

use std::process::Child;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

/// How long the sweep of one session may last when everything is ending.
const WAIT: Duration = Duration::from_millis(300);

static LIVE: Mutex<Vec<libc::pid_t>> = Mutex::new(Vec::new());

/// A command's session, listed until this is dropped (after the command is reaped).
#[must_use = "the session is unlisted when this is dropped"]
pub struct Tracked(libc::pid_t);

/// List the session `child` leads.
pub fn track(child: &Child) -> Tracked {
    let leader = child.id().cast_signed();
    LIVE.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(leader);
    Tracked(leader)
}

impl Drop for Tracked {
    fn drop(&mut self) {
        let mut live = LIVE.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = live.iter().position(|pid| *pid == self.0) {
            live.swap_remove(at);
        }
    }
}

/// Kill every listed session with everything in it. Not for a signal handler: it locks and
/// lists processes.
pub fn kill_all() {
    let live: Vec<libc::pid_t> = LIVE.lock().unwrap_or_else(PoisonError::into_inner).clone();
    kill(&live);
}

/// Kill the sessions led by `leaders`.
fn kill(leaders: &[libc::pid_t]) {
    for &leader in leaders {
        // SAFETY: kill only sends a signal; it touches nothing in this process.
        unsafe { libc::kill(-leader, libc::SIGKILL) };
        super::sweep::end(leader, WAIT);
        // SAFETY: as above; the leader itself, when the group's signal missed it.
        unsafe { libc::kill(leader, libc::SIGKILL) };
    }
}

#[cfg(test)]
#[path = "leaders_tests.rs"]
mod tests;
