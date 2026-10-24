//! Every interface attached to the sessions, and what they were told (W5, W6). An event goes to
//! every client attached and, unless it is live only, into a bounded ring under its number, so a
//! client that attaches later gets what came after its cursor, then the questions still waiting.
//! A client whose channel is closed is dropped; none is waited for (the socket cuts slow ones).
//!
//! Frames leave in the order they were posted, the same for every client, yet no lock is held
//! while one is sent (R6): posting appends to an outbox under the lock, and one thread at a time
//! (whoever finds no one else at it) empties it outside the lock.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard, PoisonError};

use protocol::{Event, ToClient};

/// An attached client, by the hub that knows it.
pub(crate) type ClientId = u64;

/// Events kept for a client that attaches later.
pub(crate) const KEPT: usize = 2_000;

/// Whether `event` is worth giving again to a client that attaches. A hands' step is drawn as a
/// passing line (W6); a copy, a quit, a prefill and a list are what one client asked for once.
/// The rest is replayed: a queue or a total is sent whole, so the last replayed is right.
fn replayable(event: &Event) -> bool {
    !matches!(
        event,
        Event::Step(_) | Event::Copy(_) | Event::Quit | Event::Prefill(_) | Event::Saved(_)
    )
}

struct Client {
    id: ClientId,
    out: Sender<ToClient>,
    /// The last post it has from its replay: what was posted to all before is in it already.
    since: u64,
}

/// A frame posted: its post's number, for whom, and the frame.
type Post = (u64, To, ToClient);
/// A client as a flush sees it: its id, its channel, and its last post replayed.
type Seen = (ClientId, Sender<ToClient>, u64);

#[derive(Clone, Copy)]
enum To {
    All,
    One(ClientId),
}

#[derive(Default)]
struct State {
    clients: Vec<Client>,
    /// The numbered events kept, oldest first.
    ring: VecDeque<(u64, ToClient)>,
    room: usize,
    seq: u64,
    /// The questions waiting for a reply, in the order asked.
    asked: Vec<(u64, ToClient)>,
    /// What is posted and not yet sent, each with its post's number.
    outbox: VecDeque<Post>,
    posted: u64,
    next_client: ClientId,
}

pub(crate) struct Hub {
    state: Mutex<State>,
    sending: AtomicBool,
}

impl Hub {
    pub(crate) fn new(room: usize) -> Self {
        let state = State {
            room,
            ..State::default()
        };
        Self {
            state: Mutex::new(state),
            sending: AtomicBool::new(false),
        }
    }

    /// `frame` to every client: an event numbered and kept, a question kept until resolved.
    pub(crate) fn send(&self, frame: ToClient) {
        {
            let mut state = self.lock();
            let frame = state.keep(frame);
            state.post(To::All, frame);
        }
        self.flush();
    }

    /// `frame` to client `id` alone (what came of its command), in order with the rest.
    pub(crate) fn to(&self, id: ClientId, frame: ToClient) {
        self.lock().post(To::One(id), frame);
        self.flush();
    }

    /// Attach `out`: first what came after `cursor`, then the questions waiting, then all that
    /// happens. Its id, and the number its replay went up to.
    pub(crate) fn attach(&self, out: Sender<ToClient>, cursor: Option<u64>) -> (ClientId, u64) {
        let attached = {
            let mut state = self.lock();
            let id = state.next_client;
            state.next_client += 1;
            let seq = state.replay(id, cursor);
            let since = state.posted;
            state.clients.push(Client { id, out, since });
            (id, seq)
        };
        self.flush();
        attached
    }

    /// Client `id` again from `cursor`, as [`Hub::attach`] does; `None` once it is gone.
    pub(crate) fn rewind(&self, id: ClientId, cursor: Option<u64>) -> Option<u64> {
        let seq = {
            let mut state = self.lock();
            let at = state.clients.iter().position(|client| client.id == id)?;
            let seq = state.replay(id, cursor);
            state.clients[at].since = state.posted;
            seq
        };
        self.flush();
        Some(seq)
    }

    /// Client `id` hears nothing more; how many are still attached.
    pub(crate) fn detach(&self, id: ClientId) -> usize {
        let mut state = self.lock();
        state.clients.retain(|client| client.id != id);
        state.clients.len()
    }

    pub(crate) fn attached(&self) -> usize {
        self.lock().clients.len()
    }

    /// Send what is posted, unless another thread is at it: it sends ours too. Checked again
    /// after letting go, for what was posted while it finished.
    fn flush(&self) {
        while !self.sending.swap(true, Ordering::AcqRel) {
            while let Some((batch, clients)) = self.take() {
                let gone = send(batch, &clients);
                if !gone.is_empty() {
                    self.lock()
                        .clients
                        .retain(|client| !gone.contains(&client.id));
                }
            }
            self.sending.store(false, Ordering::Release);
            let empty = self.lock().outbox.is_empty();
            if empty {
                return;
            }
        }
    }

    /// What is posted, and who is attached as it was posted.
    fn take(&self) -> Option<(Vec<Post>, Vec<Seen>)> {
        let mut state = self.lock();
        if state.outbox.is_empty() {
            return None;
        }
        let batch = state.outbox.drain(..).collect();
        let clients = (state.clients.iter())
            .map(|client| (client.id, client.out.clone(), client.since))
            .collect();
        Some((batch, clients))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    fn post(&mut self, to: To, frame: ToClient) {
        self.posted += 1;
        self.outbox.push_back((self.posted, to, frame));
    }

    /// `frame` as it is sent: an event numbered and kept unless live only; a question kept
    /// while it waits.
    fn keep(&mut self, frame: ToClient) -> ToClient {
        match frame {
            ToClient::Event { session, event, .. } if replayable(&event) => {
                self.seq += 1;
                let seq = Some(self.seq);
                let frame = ToClient::Event {
                    session,
                    event,
                    seq,
                };
                self.ring.push_back((self.seq, frame.clone()));
                while self.ring.len() > self.room {
                    self.ring.pop_front();
                }
                frame
            }
            ToClient::Ask { id, .. } => {
                self.asked.push((id, frame.clone()));
                frame
            }
            ToClient::Resolved { id } => {
                self.asked.retain(|(of, _)| *of != id);
                frame
            }
            frame => frame,
        }
    }

    /// Post to `id` what came after `cursor`, a gap first when some of it is no longer kept or
    /// the cursor is not this server's, then the questions waiting. The last number replayed.
    fn replay(&mut self, id: ClientId, cursor: Option<u64>) -> u64 {
        let stale = cursor.is_some_and(|cursor| cursor > self.seq);
        let from = match stale {
            true => 0,
            false => cursor.unwrap_or(0),
        };
        let oldest = self.ring.front().map_or(self.seq + 1, |(seq, _)| *seq);
        let mut frames = Vec::new();
        if stale || from + 1 < oldest {
            frames.push(ToClient::Gap);
        }
        frames.extend(
            (self.ring.iter())
                .filter(|(seq, _)| *seq > from)
                .map(|(_, frame)| frame.clone()),
        );
        frames.extend(self.asked.iter().map(|(_, frame)| frame.clone()));
        for frame in frames {
            self.post(To::One(id), frame);
        }
        self.seq
    }
}

/// Each frame to whom it is for; the clients whose channel is closed.
fn send(batch: Vec<Post>, clients: &[Seen]) -> Vec<ClientId> {
    let mut gone = Vec::new();
    for (posted, to, frame) in batch {
        for (id, out, since) in clients {
            let wanted = match to {
                To::All => posted > *since,
                To::One(one) => one == *id,
            };
            if wanted && !gone.contains(id) && out.send(frame.clone()).is_err() {
                gone.push(*id);
            }
        }
    }
    gone
}

#[cfg(test)]
#[path = "hub_tests.rs"]
mod tests;
