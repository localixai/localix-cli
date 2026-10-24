//! What the server does with what an interface sends: each reply goes to the question it
//! answers, each command is done and said done (W1: every command gets one answer). A command
//! never waits on a session: what a lane must do is posted to it. Any number of connections
//! serve one peer, each its own client of it (W5, W6).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use protocol::{Command, Done, ToClient, ToServer};

use super::hub::ClientId;
use super::ui::{FIRST, Peer};
use crate::desk::Request;

/// What the server tells an interface of itself when it attaches.
pub(crate) struct Info {
    pub max_lanes: usize,
    /// Terminal sessions running, in every lane.
    pub shells: fn() -> usize,
    /// What `/` offers, and the modes, each with what it does.
    pub commands: Vec<(String, String)>,
    pub modes: Vec<(String, String)>,
}

/// Who a connection is to the peer: a client attached, or one that has not said `Attach` yet,
/// with its own channel for what its commands come to.
enum Link {
    Attached(ClientId),
    Waiting(Sender<ToClient>),
}

/// Serve the peer's first client ([`Peer::new`]'s) until it is gone: replies to their
/// questions, lanes' commands to the desk, the person's entries to the lanes' queues.
pub(crate) fn serve(
    incoming: &Receiver<ToServer>,
    peer: &Arc<Peer>,
    desk: &Sender<Request>,
    info: &Info,
) {
    serving(incoming, peer, Link::Attached(FIRST), desk, info);
}

/// Serve a later connection to `peer`, whose frames go to `out`: it hears the sessions once it
/// says `Attach` with its cursor, and is detached when it is gone.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "D4: `llx serve`'s later connections, hosted next")
)]
pub(crate) fn connection(
    incoming: &Receiver<ToServer>,
    peer: &Arc<Peer>,
    out: Sender<ToClient>,
    desk: &Sender<Request>,
    info: &Info,
) {
    serving(incoming, peer, Link::Waiting(out), desk, info);
}

fn serving(
    incoming: &Receiver<ToServer>,
    peer: &Peer,
    mut link: Link,
    desk: &Sender<Request>,
    info: &Info,
) {
    for frame in incoming {
        match frame {
            ToServer::Reply { id, reply } => peer.replied(id, reply),
            ToServer::Call {
                id,
                command: Command::Attach { cursor },
            } => {
                let (attached, done) = attach(peer, link, cursor);
                link = Link::Attached(attached);
                peer.to(attached, ToClient::Done { id, done });
            }
            ToServer::Call { id, command } => {
                let done = ToClient::Done {
                    id,
                    done: done(command, peer, desk, info),
                };
                match &link {
                    Link::Attached(me) => peer.to(*me, done),
                    Link::Waiting(out) => {
                        let _ = out.send(done);
                    }
                }
            }
        }
    }
    // The last interface is gone: nobody will answer what waits for the person.
    if let Link::Attached(me) = link
        && peer.detach(me) == 0
    {
        peer.inputs.leave();
    }
}

/// The connection attached from `cursor`: replayed to, if it was attached already.
fn attach(peer: &Peer, link: Link, cursor: Option<u64>) -> (ClientId, Done) {
    match link {
        Link::Waiting(out) => {
            let (attached, seq) = peer.attach(out, cursor);
            (attached, Done::Attached { seq })
        }
        Link::Attached(me) => match peer.rewind(me, cursor) {
            Some(seq) => (me, Done::Attached { seq }),
            None => (
                me,
                Done::Failed {
                    why: "this client was dropped".into(),
                },
            ),
        },
    }
}

fn done(command: Command, peer: &Peer, desk: &Sender<Request>, info: &Info) -> Done {
    let to_desk = |request| match desk.send(request) {
        Ok(()) => Done::Ok,
        Err(_gone) => Done::Failed {
            why: "the sessions have ended".into(),
        },
    };
    match command {
        Command::Initialize { place, .. } => {
            // The sessions this client opens work where it is.
            if let Some(place) = place {
                let _ = desk.send(Request::Attach(place));
            }
            Done::Initialized {
                version: protocol::VERSION,
                max_lanes: info.max_lanes,
                commands: info.commands.clone(),
                modes: info.modes.clone(),
            }
        }
        Command::Input(input) => {
            peer.inputs.input(input);
            Done::Ok
        }
        Command::Interrupt { session } => Done::Interrupted {
            again: crate::interrupt::stop(&session),
        },
        Command::Halt { id } => {
            crate::interrupt::stop(&id);
            Done::Ok
        }
        Command::StopJob { lane, job } => {
            crate::interrupt::stop(&crate::interrupt::job_key(&lane, &job));
            Done::Ok
        }
        Command::Close { session } => {
            peer.inputs.close(&session);
            Done::Ok
        }
        Command::Open { target } => to_desk(Request::Open(target)),
        Command::List => to_desk(Request::List),
        Command::QuitAll => to_desk(Request::Quit),
        Command::Leave => {
            peer.inputs.leave();
            Done::Ok
        }
        Command::Work => Done::Work {
            shells: (info.shells)(),
        },
        // Served by `serving`, which knows the connection.
        Command::Attach { .. } => Done::Ok,
    }
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
