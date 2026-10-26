//! The daemon as a client sees it (docs/daemon.md): its build, the interfaces attached, the
//! sessions moved to the background waiting for the next, and its end when one asks for it. In
//! process there is none: `serve` refuses what only a daemon does.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};

use protocol::Done;

use super::Peer;
use super::toolbox::Owners;
use crate::desk::Request;
use crate::output::lock;

/// Sessions moved to the background in a directory: their peer, and their desk's requests.
pub(crate) struct Kept {
    pub cwd: String,
    pub peer: Arc<Peer>,
    pub desk: Sender<Request>,
    /// The numbering of its terminal sessions, which quitting them would stop.
    pub terminals: Arc<AtomicU32>,
}

#[cfg_attr(not(unix), expect(dead_code, reason = "the daemon is unix only"))]
pub(crate) struct Host {
    pub(crate) build: String,
    attached: AtomicUsize,
    kept: Mutex<Vec<Kept>>,
    /// The tool clients' toolboxes, by owner, kept between their calls (W9).
    owners: Arc<Owners>,
    ended: Mutex<bool>,
    ending: Condvar,
}

/// An interface attached while this lives: held by its connection, not its desk, which may go on
/// in the background once the connection ends.
pub(crate) struct Attached(Arc<Host>);

impl Drop for Attached {
    fn drop(&mut self) {
        self.0.attached.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg_attr(not(unix), expect(dead_code, reason = "the daemon is unix only"))]
impl Host {
    pub(crate) fn new(build: String) -> Arc<Self> {
        Arc::new(Self {
            build,
            attached: AtomicUsize::new(0),
            kept: Mutex::default(),
            owners: Arc::default(),
            ended: Mutex::new(false),
            ending: Condvar::new(),
        })
    }

    pub(crate) fn attach(self: &Arc<Self>) -> Attached {
        self.attached.fetch_add(1, Ordering::SeqCst);
        Attached(Arc::clone(self))
    }

    pub(crate) fn owners(&self) -> Arc<Owners> {
        Arc::clone(&self.owners)
    }

    /// Sessions moved to the background, for the next interface in their directory.
    pub(crate) fn keep(&self, kept: Kept) {
        lock(&self.kept).push(kept);
    }

    /// The sessions moved to the background in `cwd`, taken for an interface attaching there.
    pub(crate) fn take(&self, cwd: &str) -> Option<Kept> {
        let mut kept = lock(&self.kept);
        let at = kept.iter().position(|kept| kept.cwd == cwd)?;
        Some(kept.remove(at))
    }

    /// `peer`'s sessions ended: nothing is kept of them.
    pub(crate) fn forget(&self, peer: &Arc<Peer>) {
        lock(&self.kept).retain(|kept| !Arc::ptr_eq(&kept.peer, peer));
    }

    /// What `llx serve status` says.
    pub(crate) fn status(&self) -> Done {
        Done::Status {
            build: self.build.clone(),
            pid: std::process::id(),
            attached: self.attached.load(Ordering::SeqCst),
            lanes: crate::desk::open_anywhere(),
            working: crate::interrupt::working(),
            // An owner's tabs wait for its next call as sessions moved there wait for theirs.
            kept: lock(&self.kept).len() + self.owners.holding(),
            jobs: crate::interrupt::jobs(),
        }
    }

    /// Asked to end: [`Host::wait`] returns.
    pub(crate) fn end(&self) {
        *lock(&self.ended) = true;
        self.ending.notify_all();
    }

    /// Until the daemon is asked to end.
    pub(crate) fn wait(&self) {
        let mut ended = lock(&self.ended);
        while !*ended {
            ended = self
                .ending
                .wait(ended)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }
}
