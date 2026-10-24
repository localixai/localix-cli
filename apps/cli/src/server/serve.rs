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
    /// The commands of the terminal sessions running, in every lane.
    pub shells: fn() -> Vec<String>,
    /// What `/` offers, and the modes, each with what it does.
    pub commands: Vec<(String, String)>,
    pub modes: Vec<(String, String)>,
    /// Whether work may go on once the interface leaves (`Command::Background`).
    pub background: bool,
}

/// Serve `incoming` until the interface is gone: replies to their questions, lanes' commands to
/// the desk, the person's entries to the lanes' queues. Whether the person moved the session to
/// the background: then its prompts wait for the next interface. Otherwise they end with nothing,
/// and the desk, its requests' sender dropped with this connection, stops the lanes' work.
pub(crate) fn serve(
    incoming: &Receiver<ToServer>,
    peer: &Arc<Peer>,
    desk: &Sender<Request>,
    info: &Info,
) -> bool {
    let mut kept = false;
    for frame in incoming {
        match frame {
            ToServer::Reply { id, reply } => peer.replied(id, reply),
            ToServer::Call { id, command } => {
                kept |= command == Command::Background;
                let done = done(command, peer, desk, info);
                peer.send(ToClient::Done { id, done });
            }
        }
    }
    if !kept {
        // The interface is gone: nobody will answer what waits for the person.
        peer.inputs.leave();
    }
    kept
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
                background: info.background,
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
        Command::Work => {
            let shells = (info.shells)();
            Done::Work {
                shells: shells.len(),
                running: peer.work.running(shells),
            }
        }
        Command::Background => to_desk(Request::Background),
    }
}

#[cfg(test)]
#[path = "serve_tests.rs"]
mod tests;
