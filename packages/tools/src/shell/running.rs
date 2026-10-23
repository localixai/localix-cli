//! One running session: the wait on its output, and its end. Each session's id (its shell's pid,
//! which is also its first process group's) is mirrored in a static table, so a signal handler
//! that quits at once can still kill them all (see [`kill_all_now`]).

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

use super::process::{self, Ending};
use super::pty::Pty;
use super::session::{Turn, Until, Wait};
use super::sweep;

/// How long the output must stay quiet for the program to be taken as waiting.
const QUIET: Duration = Duration::from_millis(300);
/// The longest sleep of a wait: only so a stop and an exit are noticed, output wakes it at once.
const POLL: Duration = Duration::from_millis(50);
/// How long the end of the output is waited for once the program exited, or was killed: a
/// process it started may still hold the terminal.
const DRAIN: Duration = Duration::from_millis(100);
/// Slots for every registry's sessions: `MAX_SESSIONS` each, for the sessions of the interface.
const KILL_SLOTS: usize = 32;
/// The sessions' ids, for [`kill_all_now`]: a signal handler may read only atomics.
static GROUPS: [AtomicI32; KILL_SLOTS] = [const { AtomicI32::new(0) }; KILL_SLOTS];

pub struct Session {
    pub id: u32,
    pub command: String,
    started: Instant,
    pub pty: Pty,
    /// Where its group is mirrored in [`GROUPS`].
    slot: Option<usize>,
    ended: bool,
}

impl Session {
    pub fn new(id: u32, command: String, pty: Pty) -> Self {
        let slot = mirror(pty.child.id());
        Self {
            id,
            command,
            started: Instant::now(),
            pty,
            slot,
            ended: false,
        }
    }

    pub fn wait(&mut self, wait: Wait, stop: &(dyn Fn() -> bool + Sync)) -> Until {
        let deadline = Instant::now() + wait.limit();
        let shared = Arc::clone(&self.pty.shared);
        let mut stream = shared.lock();
        // Every condition is checked before sleeping: an answer already there costs no time.
        loop {
            if stream.matched {
                return Until::Matched;
            }
            if self.pty.exited() {
                drop(stream);
                self.drain();
                self.end();
                let status = self.pty.child.try_wait().ok().flatten();
                return Until::Exited(status.map_or(Ending::Unknown, process::ending));
            }
            let quiet = stream.changed_at.elapsed();
            let settling = wait.needle.is_none() && stream.since_write > 0;
            if settling && quiet >= QUIET {
                return Until::Settled;
            }
            if stop() {
                return Until::Stopped;
            }
            let now = Instant::now();
            if now >= deadline {
                return Until::Deadline;
            }
            let mut pause = POLL.min(deadline - now);
            if settling {
                pause = pause.min(QUIET - quiet);
            }
            stream = match shared.wake.wait_timeout(stream, pause) {
                Ok((stream, _)) => stream,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
    }

    /// Wait at most [`DRAIN`] for the output to end.
    fn drain(&self) {
        let deadline = Instant::now() + DRAIN;
        let mut stream = self.pty.shared.lock();
        while !stream.eof {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            stream = match self.pty.shared.wake.wait_timeout(stream, left) {
                Ok((stream, _)) => stream,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
    }

    /// Kill every process of the session and reap its shell; whether everything it started is
    /// known to be gone.
    pub fn end(&mut self) -> bool {
        if self.ended {
            return true;
        }
        self.ended = true;
        // Unmirrored first: once the shell is reaped, its pid may be given to another process.
        if let Some(slot) = self.slot.take() {
            let group = self.pty.child.id() as i32;
            let _ = GROUPS[slot].compare_exchange(group, 0, Ordering::SeqCst, Ordering::SeqCst);
        }
        let killed = process::kill(&mut self.pty.child);
        self.drain();
        self.pty.shut();
        killed
    }

    pub fn turn(&self, until: Until, waited: Duration, stalled: Option<(usize, usize)>) -> Turn {
        Turn {
            id: self.id,
            command: self.command.clone(),
            until,
            waited,
            age: self.started.elapsed(),
            output: self.pty.shared.lock().take(),
            stalled,
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.end();
    }
}

/// Kill every session at once, its shell's group and then the rest of it: for a signal handler
/// about to exit, so only atomics and system calls on the stack (`kill`, and the sweep's).
pub fn kill_all_now() {
    for slot in &GROUPS {
        let group = slot.swap(0, Ordering::SeqCst);
        if group > 0 {
            // SAFETY: kill only sends a signal; it touches nothing in this process.
            unsafe { libc::kill(-group, libc::SIGKILL) };
            // The shell is not reaped (it is this process's child, and nothing waits for it
            // here), so its pid still names the session. Twice: a job may fork as it is killed.
            sweep::kill_members(group);
            sweep::kill_members(group);
        }
    }
}

/// How many sessions are running, in every registry of the process.
pub fn running_anywhere() -> usize {
    GROUPS
        .iter()
        .filter(|slot| slot.load(Ordering::SeqCst) != 0)
        .count()
}

/// Mirror `group` in a free slot of [`GROUPS`]; with none free, only the normal ending kills it.
fn mirror(group: u32) -> Option<usize> {
    let group = group as i32;
    GROUPS.iter().position(|slot| {
        slot.compare_exchange(0, group, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    })
}
