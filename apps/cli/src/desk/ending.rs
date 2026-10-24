//! How the desk's lanes end: each leaves the table as it ends, however it ends; a panic ends the
//! desk; the desk's own end stops every lane; and quitting while lanes work stops them all and
//! waits a bounded time before the process ends anyway.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::output::lock;
use crate::server::Ui;
use protocol::Event;

/// How long "Exit and stop tasks" waits for the lanes it stopped.
pub(super) const QUIT_WAIT: Duration = Duration::from_secs(2);

/// The lanes open, by session id, oldest first.
#[derive(Default)]
pub(super) struct Lanes {
    live: Mutex<Vec<String>>,
    ended: Condvar,
    /// A lane panicked: the others are stopped.
    pub panicked: AtomicBool,
}

impl Lanes {
    pub(super) fn has(&self, id: &str) -> bool {
        lock(&self.live).iter().any(|live| live == id)
    }

    pub(super) fn count(&self) -> usize {
        lock(&self.live).len()
    }

    pub(super) fn join(&self, id: &str) {
        lock(&self.live).push(id.to_string());
    }

    /// Wait up to `timeout` for a lane to end.
    pub(super) fn wait(&self, timeout: Duration) {
        let live = lock(&self.live);
        let _ = self.ended.wait_timeout(live, timeout);
    }

    /// Wait up to `timeout` for every lane to end; whether they did.
    pub(super) fn wait_empty(&self, timeout: Duration) -> bool {
        let end = Instant::now() + timeout;
        let mut live = lock(&self.live);
        while !live.is_empty() {
            let Some(left) = end.checked_duration_since(Instant::now()) else {
                return false;
            };
            live = match self.ended.wait_timeout(live, left) {
                Ok((live, _)) => live,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
        true
    }

    /// Stop every lane's task.
    pub(super) fn stop_all(&self) {
        for id in lock(&self.live).iter() {
            crate::interrupt::stop(id);
        }
    }

    /// "Exit and stop tasks": every lane is stopped; those that do not end within `bound` are left to
    /// `quit`, which ends the process.
    pub(super) fn quit(&self, bound: Duration, quit: &dyn Fn()) {
        self.stop_all();
        if !self.wait_empty(bound) {
            quit();
        }
    }
}

/// A lane's end, however it ends: it leaves the desk. A panic ends the desk too (and with it the
/// interface), so the terminal the panic hook gave back is not left under an interface drawing.
pub(super) struct Leaving<'a> {
    pub lanes: &'a Lanes,
    pub id: String,
}

impl Drop for Leaving<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.lanes.panicked.store(true, Ordering::SeqCst);
        }
        lock(&self.lanes.live).retain(|live| *live != self.id);
        self.lanes.ended.notify_all();
    }
}

/// Stops every lane when the desk ends, however it ends, and ends the interface: a lane at its
/// prompt then gets nothing, so the scope waits only for lanes that are ending.
pub(super) struct AllLanes<'a> {
    pub lanes: &'a Lanes,
    pub ui: &'a Ui,
}

impl Drop for AllLanes<'_> {
    fn drop(&mut self) {
        self.lanes.stop_all();
        self.ui.send(Event::Quit);
    }
}

#[cfg(test)]
#[path = "ending_tests.rs"]
mod tests;
