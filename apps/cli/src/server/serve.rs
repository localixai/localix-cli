//! What the server does with what an interface sends: each reply goes to the question it
//! answers, each command is done and said done (W1: every command gets one answer). A command
//! never waits on a session: what a lane must do is posted to it.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use protocol::{Command, Done, ToClient, ToServer};

use super::ui::Peer;
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

/// Serve `incoming` until the interface is gone: replies to their questions, lanes' commands to
/// the desk, the person's entries to the lanes' queues.
pub(crate) fn serve(
    incoming: &Receiver<ToServer>,
    peer: &Arc<Peer>,
    desk: &Sender<Request>,
    info: &Info,
) {
    for frame in incoming {
        match frame {
            ToServer::Reply { id, reply } => peer.replied(id, reply),
            ToServer::Call { id, command } => {
                let done = done(command, peer, desk, info);
                peer.send(ToClient::Done { id, done });
            }
        }
    }
    // The interface is gone: nobody will answer what waits for the person.
    peer.inputs.leave();
}

fn done(command: Command, peer: &Peer, desk: &Sender<Request>, info: &Info) -> Done {
    let to_desk = |request| match desk.send(request) {
        Ok(()) => Done::Ok,
        Err(_gone) => Done::Failed {
            why: "the sessions have ended".into(),
        },
    };
    match command {
        Command::Initialize { .. } => Done::Initialized {
            version: protocol::VERSION,
            max_lanes: info.max_lanes,
            commands: info.commands.clone(),
            modes: info.modes.clone(),
        },
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
    }
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
