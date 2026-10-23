//! The interface's side of the seam (ARCHITECTURE.md W4): one [`Client`] whatever carries the
//! frames. In process the frames go over channels; over a socket (D3) the same frames go as
//! lines. A command waits for what is done with it; everything else the server sends goes to the
//! interface as it comes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use protocol::{Command, Done, Reply, ToClient, ToServer};

/// Why a command came to nothing: what carries the frames, or what the server said (W4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// The server is gone.
    Gone,
    /// No answer in time.
    Timeout,
    /// The server could not do it, and why.
    Failed(String),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gone => f.write_str("the server is gone"),
            Self::Timeout => f.write_str("the server did not answer in time"),
            Self::Failed(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for CallError {}

/// The commands sent and not yet done, each waiting where its answer goes.
type Waiting = Mutex<HashMap<u64, SyncSender<Done>>>;

/// The interface's connection to the server; cheap to clone, one per thread that calls.
#[derive(Clone)]
pub struct Client {
    to_server: Sender<ToServer>,
    waiting: Arc<Waiting>,
    next: Arc<AtomicU64>,
}

impl Client {
    /// A client over channels to a server in this process: frames from the server that are not
    /// what a command came to go to `deliver`, in order, until the server is gone.
    pub fn in_process(
        to_server: Sender<ToServer>,
        from_server: Receiver<ToClient>,
        deliver: impl Fn(ToClient) + Send + 'static,
    ) -> std::io::Result<Self> {
        let waiting = Arc::new(Waiting::default());
        let theirs = Arc::clone(&waiting);
        std::thread::Builder::new()
            .name("llx-client".into())
            .spawn(move || {
                for frame in from_server {
                    match frame {
                        ToClient::Done { id, done } => {
                            let reply = lock(&theirs).remove(&id);
                            if let Some(reply) = reply {
                                let _ = reply.send(done);
                            }
                        }
                        frame => deliver(frame),
                    }
                }
                // The server is gone: what waits for it ends now.
                lock(&theirs).clear();
            })?;
        Ok(Self {
            to_server,
            waiting,
            next: Arc::new(AtomicU64::new(1)),
        })
    }

    /// `command`, and what was done with it, within `wait`.
    pub fn call(&self, command: Command, wait: Duration) -> Result<Done, CallError> {
        let (reply, done) = sync_channel(1);
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        lock(&self.waiting).insert(id, reply);
        if self.to_server.send(ToServer::Call { id, command }).is_err() {
            lock(&self.waiting).remove(&id);
            return Err(CallError::Gone);
        }
        let done = match done.recv_timeout(wait) {
            Ok(done) => done,
            Err(RecvTimeoutError::Timeout) => {
                lock(&self.waiting).remove(&id);
                return Err(CallError::Timeout);
            }
            Err(RecvTimeoutError::Disconnected) => return Err(CallError::Gone),
        };
        match done {
            Done::Failed { why } => Err(CallError::Failed(why)),
            done => Ok(done),
        }
    }

    /// `command`, not waiting for what is done with it.
    pub fn send(&self, command: Command) -> Result<(), CallError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        (self.to_server.send(ToServer::Call { id, command })).map_err(|_gone| CallError::Gone)
    }

    /// The person's reply to the question `id`.
    pub fn reply(&self, id: u64, reply: Reply) -> Result<(), CallError> {
        (self.to_server.send(ToServer::Reply { id, reply })).map_err(|_gone| CallError::Gone)
    }
}

/// The table, whatever a thread that held it did: it is only a map of waiting channels.
fn lock(waiting: &Waiting) -> MutexGuard<'_, HashMap<u64, SyncSender<Done>>> {
    waiting.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
