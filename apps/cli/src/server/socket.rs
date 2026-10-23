//! The server's end of the socket (W1, W4, W6): a listener in the person's own directory, and
//! for each connection the same two ends as in process, a stream of frames in and a sender out,
//! so `serve` cannot tell them apart. A line that is not a frame is dropped and said (W8); a
//! client that does not read is cut off when its bounded queue fills, never waited for (W6).

use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, SyncSender, TrySendError, sync_channel};
use std::thread::JoinHandle;

use protocol::{Event, ToClient, ToServer};

/// Frames waiting for a client before it is taken for one that does not read.
const ROOM: usize = 1024;
/// Frames read from a client and not yet served: past this, its reader waits, and it does.
const INCOMING: usize = 64;

/// One client's ends, as `serve` and `Peer::new` take them.
pub(crate) struct Connection {
    pub(crate) incoming: Receiver<ToServer>,
    pub(crate) out: Sender<ToClient>,
}

/// A listener at `path`, in a directory only the person may enter, the socket theirs alone.
/// Whatever was at `path` is replaced: the caller holds the daemon's lock, so it was stale.
pub(crate) fn bind(path: &Path) -> std::io::Result<UnixListener> {
    if let Some(dir) = path.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        // A directory made earlier, by hand or an older llx, is closed again.
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Accept on a thread of its own; each connection's ends go to `each`, on a thread of their own.
pub(crate) fn accept(
    listener: UnixListener,
    each: impl Fn(Connection) + Send + Sync + Clone + 'static,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("llx-accept".into())
        .spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let each = each.clone();
                let served = std::thread::Builder::new()
                    .name("llx-connection".into())
                    .spawn(move || {
                        if let Ok(connection) = connection(stream) {
                            each(connection);
                        }
                    });
                // No thread for it: the connection drops, and the client sees it gone.
                drop(served);
            }
        })
}

/// A connection's ends, with its three threads: the reader, the writer, and between `out` and
/// the writer a pump that never blocks, so a slow client fills only its own bounded queue.
pub(crate) fn connection(stream: UnixStream) -> std::io::Result<Connection> {
    let (to_server, incoming) = sync_channel(INCOMING);
    #[allow(
        clippy::disallowed_methods,
        reason = "the pump empties it as fast as the server fills it, into a bounded queue"
    )]
    let (out, outgoing) = std::sync::mpsc::channel();
    let (to_writer, writing) = sync_channel(ROOM);
    let reading = stream.try_clone()?;
    let cut = stream.try_clone()?;
    let told = out.clone();
    let spawn = |name: &str| std::thread::Builder::new().name(name.into());
    spawn("llx-conn-reader").spawn(move || read(reading, &to_server, &told))?;
    spawn("llx-conn-writer").spawn(move || write(stream, &writing))?;
    spawn("llx-conn-pump").spawn(move || pump(&outgoing, &to_writer, &cut))?;
    Ok(Connection { incoming, out })
}

/// Each line as a frame to serve; one that is not a frame is dropped, and the client is told.
/// When the client hangs up, the connection is over both ways.
fn read(stream: UnixStream, to_server: &SyncSender<ToServer>, told: &Sender<ToClient>) {
    let shut = stream.try_clone();
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { break };
        match serde_json::from_str(&line) {
            Ok(frame) => {
                if to_server.send(frame).is_err() {
                    break;
                }
            }
            Err(error) => {
                let event = Event::Note(format!("a line that is not a frame was dropped: {error}"));
                let _ = told.send(ToClient::Event {
                    session: None,
                    event,
                });
            }
        }
    }
    if let Ok(shut) = shut {
        let _ = shut.shutdown(Shutdown::Both);
    }
}

/// Frames into the writer's queue; a full one means the client does not read, and it is cut off.
fn pump(outgoing: &Receiver<ToClient>, to_writer: &SyncSender<ToClient>, cut: &UnixStream) {
    for frame in outgoing {
        match to_writer.try_send(frame) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                let _ = cut.shutdown(Shutdown::Both);
                return;
            }
            Err(TrySendError::Disconnected(_)) => return,
        }
    }
}

/// Each frame as one line, until the connection ends.
fn write(mut stream: UnixStream, writing: &Receiver<ToClient>) {
    for frame in writing {
        let Ok(mut line) = serde_json::to_vec(&frame) else {
            continue;
        };
        line.push(b'\n');
        if stream.write_all(&line).is_err() {
            break;
        }
    }
    let _ = stream.shutdown(Shutdown::Both);
}

#[cfg(test)]
#[path = "socket_tests.rs"]
pub(crate) mod tests;
