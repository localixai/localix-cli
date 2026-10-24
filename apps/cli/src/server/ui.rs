//! The session's side of the seam: what it reports, and how it asks and waits. Each report is a
//! frame to the interface, in the session's name; each question goes out with an id and waits
//! for the reply under it (W5), and a stopped task withdraws it. Entries come from the server's
//! own queue ([`Inputs`]), never from the interface's state.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use protocol::{Adjustment, Approval, Asked, Event, Reply, ToClient};

use super::hub::{ClientId, Hub};
use super::inputs::{Inputs, Typed};

/// The client [`Peer::new`] attaches.
pub(crate) const FIRST: ClientId = 0;

/// How often a waiting question checks whether the task was stopped.
const POLL: Duration = Duration::from_millis(100);

/// What the server holds of its interfaces: every client attached and what they were told
/// ([`Hub`]), the questions waiting for a reply, and every lane's entries.
pub(crate) struct Peer {
    hub: Hub,
    asked: Mutex<HashMap<u64, SyncSender<Reply>>>,
    next: AtomicU64,
    pub(crate) inputs: Inputs,
}

impl Peer {
    /// A peer with one client attached, `out`, as its first ([`FIRST`]).
    pub(crate) fn new(out: Sender<ToClient>) -> Arc<Self> {
        let peer = Arc::new_cyclic(|peer: &Weak<Self>| {
            let told = Weak::clone(peer);
            let inputs = Inputs::new(move |lane, event| {
                if let Some(peer) = told.upgrade() {
                    let session = Some(lane.to_string());
                    peer.send(ToClient::Event {
                        session,
                        event,
                        seq: None,
                    });
                }
            });
            Self {
                hub: Hub::new(super::hub::KEPT),
                asked: Mutex::default(),
                next: AtomicU64::new(1),
                inputs,
            }
        });
        peer.hub.attach(out, None);
        peer
    }

    /// Attach another client: what came after `cursor` first, then the questions waiting
    /// (W5, W6). Its id, and the number its replay went up to.
    pub(crate) fn attach(&self, out: Sender<ToClient>, cursor: Option<u64>) -> (ClientId, u64) {
        self.hub.attach(out, cursor)
    }

    /// Client `id` again from `cursor`; `None` once it is gone.
    pub(crate) fn rewind(&self, id: ClientId, cursor: Option<u64>) -> Option<u64> {
        self.hub.rewind(id, cursor)
    }

    /// Client `id` hears nothing more; how many are still attached.
    pub(crate) fn detach(&self, id: ClientId) -> usize {
        self.hub.detach(id)
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "D4: whether a backgrounded desk has an interface")
    )]
    pub(crate) fn attached(&self) -> usize {
        self.hub.attached()
    }

    /// The person's reply to question `id`: the first wins, and every client is told it no
    /// longer waits (W5); a later one is let go.
    pub(crate) fn replied(&self, id: u64, reply: Reply) {
        let waiting = self.waiting().remove(&id);
        if let Some(waiting) = waiting {
            let _ = waiting.send(reply);
            self.send(ToClient::Resolved { id });
        }
    }

    /// `frame` to every client attached.
    pub(crate) fn send(&self, frame: ToClient) {
        self.hub.send(frame);
    }

    /// `frame` to client `id` alone.
    pub(crate) fn to(&self, id: ClientId, frame: ToClient) {
        self.hub.to(id, frame);
    }

    fn waiting(&self) -> MutexGuard<'_, HashMap<u64, SyncSender<Reply>>> {
        self.asked.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The session's side of the interface: cheap to clone, one per thing that reports or asks.
#[derive(Clone)]
pub(crate) struct Ui {
    peer: Arc<Peer>,
    /// The session whose reports these are; `None` for the server's own.
    lane: Option<Arc<str>>,
}

impl Ui {
    pub(crate) fn new(peer: Arc<Peer>) -> Self {
        Self { peer, lane: None }
    }

    /// The same interface, for what session `id` (a lane or an agent) sends.
    pub(crate) fn for_lane(&self, id: &str) -> Self {
        Self {
            peer: Arc::clone(&self.peer),
            lane: Some(id.into()),
        }
    }

    fn session(&self) -> String {
        self.lane.as_deref().unwrap_or_default().to_string()
    }

    /// Report `event`, in the session's name; a lane's queue learns when its task runs.
    pub(crate) fn send(&self, event: Event) {
        if let Some(lane) = self.lane.as_deref() {
            match &event {
                Event::TaskStarted(_) => self.peer.inputs.working(lane, true),
                Event::TaskEnded { .. } => self.peer.inputs.working(lane, false),
                _ => {}
            }
        }
        let session = self.lane.as_deref().map(str::to_string);
        self.peer.send(ToClient::Event {
            session,
            event,
            seq: None,
        });
    }

    /// The person's next entry; `None` when they leave or the lane is closed.
    pub(crate) fn next_entry(&self) -> Option<Typed> {
        self.peer.inputs.next(&self.session())
    }

    /// What came back from the background while nothing runs: a task of its own, after what
    /// the person queued.
    pub(crate) fn background(&self, text: &str) {
        self.peer.inputs.background(&self.session(), text);
    }

    /// What the person sent to the task running, taken (and shown as theirs) or, with `peek`,
    /// only looked at.
    pub(crate) fn steered(&self, peek: bool) -> Vec<String> {
        self.peer.inputs.steered(&self.session(), peek)
    }

    /// A guard: when it drops, the lane's entries go with it.
    pub(crate) fn ends(&self) -> Ends {
        Ends(self.clone())
    }

    /// Whether an effect may happen; a stopped task withdraws the question, and it is a no.
    pub(crate) fn approve(
        &self,
        tool: &str,
        summary: &str,
        stopped: &dyn Fn() -> bool,
    ) -> Approval {
        let asked = Asked::Approve {
            tool: tool.into(),
            summary: summary.into(),
        };
        match self.ask(asked, stopped) {
            Some(Reply::Approval(approval)) => approval,
            _ => Approval::No(None),
        }
    }

    /// One of the model's questions; `None` when it was skipped or the task stopped.
    pub(crate) fn question(
        &self,
        question: &contracts::person::Question,
        at: usize,
        of: usize,
        stopped: &dyn Fn() -> bool,
    ) -> Option<contracts::person::Answer> {
        let question = super::wire::question(question);
        match self.ask(Asked::Question { question, at, of }, stopped) {
            Some(Reply::Question(answer)) => answer.map(super::wire::answer),
            _ => None,
        }
    }

    /// One of `options` (a label and a note each), starting on `marked`; `None` when cancelled.
    pub(crate) fn pick(
        &self,
        title: &str,
        options: Vec<(String, String)>,
        marked: Option<usize>,
    ) -> Option<usize> {
        let title = title.into();
        match self.ask(
            Asked::Choose {
                title,
                options,
                marked,
            },
            &|| false,
        ) {
            Some(Reply::Choice(picked)) => picked,
            _ => None,
        }
    }

    /// A row and its inline value, starting on `marked`; `None` when cancelled.
    pub(crate) fn pick_adjustable(
        &self,
        title: &str,
        options: Vec<(String, String)>,
        marked: Option<usize>,
        adjustments: Vec<Adjustment>,
    ) -> Option<(usize, usize)> {
        let title = title.into();
        let asked = Asked::ChooseAdjustable {
            title,
            options,
            marked,
            adjustments,
        };
        match self.ask(asked, &|| false) {
            Some(Reply::Adjusted(picked)) => picked,
            _ => None,
        }
    }

    /// A line the person types after `prompt`, shown as dots when `hidden`; `None` when
    /// cancelled, or withdrawn once `done` says so.
    pub(crate) fn line(
        &self,
        prompt: &str,
        hidden: bool,
        done: Option<&dyn Fn() -> bool>,
    ) -> Option<String> {
        let prompt = prompt.into();
        match self.ask(Asked::Line { prompt, hidden }, done.unwrap_or(&|| false)) {
            Some(Reply::Line(line)) => line,
            _ => None,
        }
    }

    /// `asked`, and its reply; `None` when the interface is gone, or once `stopped` says so,
    /// which withdraws it.
    fn ask(&self, asked: Asked, stopped: &dyn Fn() -> bool) -> Option<Reply> {
        let id = self.peer.next.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = sync_channel(1);
        self.peer.waiting().insert(id, reply);
        let session = self.session();
        self.peer.send(ToClient::Ask { id, session, asked });
        let reply = wait(&answer, stopped);
        if reply.is_none() {
            self.peer.waiting().remove(&id);
            self.peer.send(ToClient::Resolved { id });
        }
        reply
    }
}

/// The lane's end, however it ends: its queue is forgotten.
pub(crate) struct Ends(Ui);

impl Drop for Ends {
    fn drop(&mut self) {
        self.0.peer.inputs.forget(&self.0.session());
    }
}

fn wait(answer: &Receiver<Reply>, stopped: &dyn Fn() -> bool) -> Option<Reply> {
    loop {
        match answer.recv_timeout(POLL) {
            Ok(reply) => return Some(reply),
            Err(RecvTimeoutError::Disconnected) => return None,
            Err(RecvTimeoutError::Timeout) if stopped() => return None,
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
