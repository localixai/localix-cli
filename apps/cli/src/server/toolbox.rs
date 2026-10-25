//! A tool client's toolbox (ARCHITECTURE.md W9, K7): what `llx mcp` serves another agent. The
//! client says where it is in its handshake; the toolbox is the agent's own `window` there, with
//! no chat model: the client's model thinks, and the decision model carries out a `goal`. Each
//! call runs on a thread of its own and is answered when it ends, so a slow one holds no other;
//! `Halt` with the call's id stops it. Its tabs end with its connection.

mod kit;

use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use protocol::{Command, Done, ToClient, ToServer};
use tools::browser::Browser;

use self::kit::Kit;
use super::host::Host;
use crate::arrivals::Lanes;
use crate::interrupt::{self, Interrupt};
use crate::output::lock;

/// What the daemon gives a toolbox: itself, its window, the lanes the window tells what the
/// person does there, and the numbering of the client's terminal sessions.
pub(crate) struct Hosted {
    pub host: Option<Arc<Host>>,
    pub browser: Option<Browser>,
    pub lanes: Arc<Lanes>,
    pub terminals: Arc<AtomicU32>,
}

/// Whether `first`, a connection's handshake, is a tool client's: its command line is `mcp`.
pub(crate) fn wanted(first: &ToServer) -> bool {
    let ToServer::Call {
        command: Command::Initialize {
            place: Some(place), ..
        },
        ..
    } = first
    else {
        return false;
    };
    place.args.first().is_some_and(|word| word == "mcp")
}

/// Serve the tool client whose handshake is `first` until its connection ends; then what its
/// calls still do stops, and the tabs it opened close.
pub(crate) fn serve(
    first: ToServer,
    incoming: &Receiver<ToServer>,
    out: &Sender<ToClient>,
    hosted: &Hosted,
) {
    let ToServer::Call {
        id,
        command: Command::Initialize { place, build, .. },
    } = first
    else {
        return;
    };
    let _attached = hosted.host.as_ref().map(|host| host.attach());
    let built = match (place, &hosted.host) {
        (_, Some(host)) if host.build != build => Err(format!(
            "the daemon is build {}, this llx {build}",
            host.build
        )),
        (Some(place), _) => Kit::new(&place, hosted),
        (None, _) => Err("a tool client says where it is".into()),
    };
    let kit = match built {
        Ok(kit) => kit,
        Err(why) => {
            let _ = out.send(ToClient::Done {
                id,
                done: Done::Failed { why },
            });
            return;
        }
    };
    let done = Done::Initialized {
        version: protocol::VERSION,
        max_lanes: 0,
        commands: vec![],
        modes: vec![],
        background: false,
    };
    let _ = out.send(ToClient::Done { id, done });
    calls(&kit, incoming, out, hosted);
    kit.close();
}

/// Each frame of the connection, until it ends: a tool's call on a thread of its own, the rest
/// answered at once. Then every call still running is stopped, and waited for.
fn calls(kit: &Kit, incoming: &Receiver<ToServer>, out: &Sender<ToClient>, hosted: &Hosted) {
    let running: Mutex<Vec<Arc<Interrupt>>> = Mutex::default();
    std::thread::scope(|scope| {
        for frame in incoming {
            let (id, command) = match frame {
                ToServer::Call { id, command } => (id, command),
                // Nobody is asked through a tool client (K7): there is nothing to reply to.
                ToServer::Reply { .. } => continue,
            };
            let Command::Tool {
                call,
                name,
                arguments,
            } = command
            else {
                let done = answer(command, kit, hosted);
                let _ = out.send(ToClient::Done { id, done });
                continue;
            };
            // Begun before its thread starts, so a `Halt` that comes at once stops it.
            let interrupt = Arc::new(Interrupt::new());
            interrupt.begin();
            let stops = Arc::clone(&interrupt);
            let known = interrupt::register(&call, move || stops.stop());
            lock(&running).push(Arc::clone(&interrupt));
            let running = &running;
            let work = move || {
                let done = kit.call(&name, &arguments, &interrupt);
                interrupt.end();
                drop(known);
                lock(running).retain(|other| !Arc::ptr_eq(other, &interrupt));
                let _ = out.send(ToClient::Done { id, done });
            };
            let thread = std::thread::Builder::new().name("llx-tool".into());
            if let Err(error) = thread.spawn_scoped(scope, work) {
                let why = format!("the call could not start: {error}");
                let _ = out.send(ToClient::Done {
                    id,
                    done: Done::Failed { why },
                });
            }
        }
        // The client is gone: nobody reads what its calls would say.
        for interrupt in lock(&running).iter() {
            interrupt.stop();
        }
    });
}

/// What a command other than a tool's call comes to: a tool client lists the tools, stops a
/// call, asks what the daemon is; it opens no session.
fn answer(command: Command, kit: &Kit, hosted: &Hosted) -> Done {
    match command {
        Command::Tools => Done::Tools {
            tools: kit.info().into_iter().collect(),
        },
        Command::Halt { id } => {
            interrupt::stop(&id);
            Done::Ok
        }
        Command::Status => match &hosted.host {
            Some(host) => host.status(),
            None => Done::Failed {
                why: "not a daemon".into(),
            },
        },
        Command::Initialize { .. } => Done::Failed {
            why: "the handshake comes once".into(),
        },
        Command::Input(_)
        | Command::Interrupt { .. }
        | Command::StopJob { .. }
        | Command::Close { .. }
        | Command::Open { .. }
        | Command::List
        | Command::QuitAll
        | Command::Leave
        | Command::Work
        | Command::Attach { .. }
        | Command::Background
        | Command::Shutdown
        | Command::Tool { .. } => Done::Failed {
            why: "a tool client opens no session: it lists the tools, calls one, or halts a call"
                .into(),
        },
    }
}

#[cfg(test)]
#[path = "toolbox_tests.rs"]
mod tests;
