use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::{Duration, Instant};

use client::Client;
use protocol::{Approval, Asked, Command, Done, Event, Input, Reply};

use super::*;
use crate::desk::Request;
use crate::server::serve::{Info, serve};
use crate::server::{Peer, Ui};

const WAIT: Duration = Duration::from_secs(5);

/// A socket path in a directory of its own (made by `bind`), short enough for `sun_path`.
pub(crate) fn place(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("llx-s{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("d.sock")
}

/// A daemon at `path` serving each connection as the binary serves its interface: its peer
/// handed to the test, and a word when its `serve` returns.
pub(crate) struct Daemon {
    pub(crate) peers: Receiver<Arc<Peer>>,
    pub(crate) ended: Receiver<()>,
}

#[allow(
    clippy::disallowed_methods,
    reason = "the desk's requests, as the binary makes its channel"
)]
pub(crate) fn daemon(path: &Path) -> Daemon {
    let (peers, peers_out) = sync_channel(4);
    let (ended, ended_out) = sync_channel(4);
    let peers: SyncSender<Arc<Peer>> = peers;
    accept(bind(path).unwrap(), move |connection| {
        let (desk, _requests) = std::sync::mpsc::channel::<Request>();
        let peer = Peer::new(connection.out);
        let _ = peers.send(Arc::clone(&peer));
        let info = Info {
            max_lanes: 3,
            shells: Vec::new,
            commands: vec![],
            modes: vec![],
            background: false,
        };
        serve(&connection.incoming, &peer, &desk, &info);
        let _ = ended.send(());
    })
    .unwrap();
    Daemon {
        peers: peers_out,
        ended: ended_out,
    }
}

#[test]
fn the_socket_is_the_persons_alone_and_a_stale_one_is_replaced() {
    let path = place("modes");
    drop(bind(&path).unwrap());
    let _listener = bind(&path).unwrap();
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(path.parent().unwrap()), 0o700);
    assert_eq!(mode(&path), 0o600);
}

#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "what the test's interface is shown, as many frames as the test sends"
)]
fn a_client_over_the_socket_speaks_to_serve_as_one_in_process_does() {
    let path = place("proof");
    let daemon = daemon(&path);
    let (shown, seen) = std::sync::mpsc::channel();
    let client = Client::connect(&path, move |frame| {
        let _ = shown.send(frame);
    })
    .unwrap();
    let handshake = Command::Initialize {
        version: protocol::VERSION,
        place: None,
    };
    assert_eq!(
        client.call(handshake, WAIT),
        Ok(Done::Initialized {
            version: protocol::VERSION,
            max_lanes: 3,
            commands: vec![],
            modes: vec![],
            background: false,
        })
    );
    let peer = daemon.peers.recv_timeout(WAIT).unwrap();

    let input = Input {
        session: "s1".into(),
        text: "fix it".into(),
        ..Input::default()
    };
    assert_eq!(client.call(Command::Input(input), WAIT), Ok(Done::Ok));
    assert_eq!(peer.inputs.next("s1").unwrap().text, "fix it");

    let ui = Ui::new(Arc::clone(&peer)).for_lane("s1");
    let asking = std::thread::Builder::new().name("asking".into());
    let asking = asking
        .spawn(move || ui.approve("bash", "ls", &|| false))
        .unwrap();
    let id = loop {
        if let ToClient::Ask { id, session, asked } = seen.recv_timeout(WAIT).unwrap() {
            assert_eq!(session, "s1");
            assert!(matches!(asked, Asked::Approve { .. }));
            break id;
        }
    };
    client.reply(id, Reply::Approval(Approval::Always)).unwrap();
    assert_eq!(asking.join().unwrap(), Approval::Always);

    drop(client);
    daemon.ended.recv_timeout(WAIT).unwrap();
}

#[test]
fn a_line_that_is_not_a_frame_is_dropped_and_said() {
    let path = place("malformed");
    let _daemon = daemon(&path);
    let mut stream = UnixStream::connect(&path).unwrap();
    stream.write_all(b"{not json\n").unwrap();
    let call = ToServer::Call {
        id: 7,
        command: Command::Work,
    };
    let mut line = serde_json::to_vec(&call).unwrap();
    line.push(b'\n');
    stream.write_all(&line).unwrap();
    stream.set_read_timeout(Some(WAIT)).unwrap();
    let frames: Vec<ToClient> = BufReader::new(stream)
        .lines()
        .take(2)
        .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
        .collect();
    assert!(
        matches!(&frames[0], ToClient::Event { event: Event::Note(note), .. } if note.contains("dropped")),
        "{frames:?}"
    );
    let done = Done::Work {
        shells: 0,
        running: vec![],
    };
    assert_eq!(frames[1], ToClient::Done { id: 7, done });
}

#[test]
fn a_client_that_never_reads_is_cut_off_and_never_waited_for() {
    let path = place("slow");
    let daemon = daemon(&path);
    let _stream = UnixStream::connect(&path).unwrap();
    let peer = daemon.peers.recv_timeout(WAIT).unwrap();
    let started = Instant::now();
    for _ in 0..20_000 {
        peer.send(ToClient::Event {
            session: None,
            event: Event::Note("x".repeat(1_000)),
        });
    }
    assert!(started.elapsed() < WAIT, "the server waited on the client");
    daemon.ended.recv_timeout(WAIT).unwrap();
}
