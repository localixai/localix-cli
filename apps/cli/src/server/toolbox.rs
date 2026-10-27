//! A tool client's toolbox (ARCHITECTURE.md W9, K7): what `llx window` calls for another agent.
//! The client says in its handshake where it is and whose tabs it calls (its owner); the toolbox
//! is the agent's own `window` there, with no chat model: the client's model thinks, and the
//! decision model carries out a `goal`. It is kept for its owner between connections, each call
//! one, until `Release`. Each call runs on a thread of its own and is answered when it ends, so a
//! slow one holds no other; `Halt` with the call's id, or the connection's end, stops it.

mod kit;

use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use protocol::{Command, Done, Place, ToClient, ToServer};
use tools::browser::Browser;

use self::kit::Kit;
use super::host::Host;
use crate::arrivals::Lanes;
use crate::interrupt::{self, Interrupt};
use crate::sync::lock;

/// What the daemon gives a toolbox: itself, its window, the lanes the window tells what the
/// person does there, the numbering of a new owner's terminal sessions, and the owners' toolboxes.
pub(crate) struct Hosted {
    pub host: Option<Arc<Host>>,
    pub browser: Option<Browser>,
    pub lanes: Arc<Lanes>,
    pub terminals: Arc<AtomicU32>,
    pub owners: Arc<Owners>,
}

/// The toolboxes kept for their owners between calls.
#[derive(Default)]
pub(crate) struct Owners(Mutex<HashMap<String, Arc<Kit>>>);

impl Owners {
    /// `owner`'s toolbox, made at `place` when it has none.
    fn of(&self, owner: &str, place: &Place, hosted: &Hosted) -> Result<Arc<Kit>, String> {
        let mut owners = lock(&self.0);
        if let Some(kit) = owners.get(owner) {
            return Ok(Arc::clone(kit));
        }
        let kit = Arc::new(Kit::new(place, hosted)?);
        owners.insert(owner.to_string(), Arc::clone(&kit));
        Ok(kit)
    }

    /// `owner` is done: its tabs close and its terminal sessions end.
    fn release(&self, owner: &str) {
        let released = lock(&self.0).remove(owner);
        if let Some(kit) = released {
            kit.close();
        }
    }

    /// How many owners have a tab open: the daemon is in use while one has.
    pub(crate) fn holding(&self) -> usize {
        let kits: Vec<Arc<Kit>> = lock(&self.0).values().cloned().collect();
        kits.iter().filter(|kit| kit.holds()).count()
    }
}

/// Whether `first`, a connection's handshake, is a tool client's: its command line is `window`,
/// then its owner.
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
    place.args.first().is_some_and(|word| word == "window")
}

/// Serve the tool client whose handshake is `first` until its connection ends; then what its
/// calls still do stops, and its owner's tabs wait for its next call.
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
        (Some(place), _) => match place.args.get(1) {
            Some(owner) => hosted
                .owners
                .of(owner, &place, hosted)
                .map(|kit| (owner.clone(), kit)),
            None => Err("a tool client names its owner".into()),
        },
        (None, _) => Err("a tool client says where it is".into()),
    };
    let (owner, kit) = match built {
        Ok(built) => built,
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
    calls(&kit, &owner, incoming, out, hosted);
}

/// Each frame of the connection, until it ends: a tool's call on a thread of its own, the rest
/// answered at once. Then every call still running is stopped, and waited for.
fn calls(
    kit: &Kit,
    owner: &str,
    incoming: &Receiver<ToServer>,
    out: &Sender<ToClient>,
    hosted: &Hosted,
) {
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
                let done = answer(command, kit, owner, hosted);
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
/// call, ends its owner's tabs, asks what the daemon is; it opens no session.
fn answer(command: Command, kit: &Kit, owner: &str, hosted: &Hosted) -> Done {
    match command {
        Command::Tools => Done::Tools {
            tools: kit.info().into_iter().collect(),
        },
        Command::Halt { id } => {
            interrupt::stop(&id);
            Done::Ok
        }
        Command::Release => {
            hosted.owners.release(owner);
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
