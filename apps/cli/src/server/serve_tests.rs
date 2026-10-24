use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use protocol::{Approval, Asked, Input, Reply, Target};

use super::*;
use crate::server::Ui;

const WAIT: Duration = Duration::from_secs(5);

/// A server on a thread of its own: where frames go in, what comes out, what the desk is asked.
struct Served {
    to_server: Sender<ToServer>,
    out: Receiver<ToClient>,
    desk: Receiver<Request>,
    peer: Arc<Peer>,
}

#[allow(
    clippy::disallowed_methods,
    reason = "the in-process transport's channels, as the binary makes them"
)]
fn served() -> Served {
    let (to_server, incoming) = channel();
    let (out, from_server) = channel();
    let (desk, requests) = channel();
    let peer = Peer::new(out);
    let theirs = Arc::clone(&peer);
    std::thread::spawn(move || {
        let info = Info {
            max_lanes: 3,
            shells: || 2,
            commands: vec![("help".into(), "the commands".into())],
            modes: vec![("plan".into(), "plan first".into())],
        };
        serve(&incoming, &theirs, &desk, &info);
    });
    Served {
        to_server,
        out: from_server,
        desk: requests,
        peer,
    }
}

impl Served {
    fn call(&self, id: u64, command: Command) {
        self.to_server.send(ToServer::Call { id, command }).unwrap();
    }

    /// What came of command `id`, past the events before it.
    fn done(&self, id: u64) -> Done {
        loop {
            match self.out.recv_timeout(WAIT).unwrap() {
                ToClient::Done { id: of, done } if of == id => return done,
                _ => {}
            }
        }
    }
}

#[test]
fn an_interface_that_attaches_is_told_what_the_server_allows() {
    let served = served();
    served.call(
        1,
        Command::Initialize {
            version: protocol::VERSION,
            place: None,
        },
    );
    assert_eq!(
        served.done(1),
        Done::Initialized {
            version: protocol::VERSION,
            max_lanes: 3,
            commands: vec![("help".into(), "the commands".into())],
            modes: vec![("plan".into(), "plan first".into())],
        }
    );
    served.call(2, Command::Work);
    assert_eq!(served.done(2), Done::Work { shells: 2 });
}

#[test]
fn what_the_person_types_is_the_lanes_next_entry() {
    let served = served();
    let input = Input {
        session: "s1".into(),
        text: "fix it".into(),
        ..Input::default()
    };
    served.call(1, Command::Input(input));
    assert_eq!(served.done(1), Done::Ok);
    let entry = served.peer.inputs.next("s1").unwrap();
    assert_eq!(entry.text, "fix it");
}

#[test]
fn a_reply_reaches_the_question_waiting_under_its_id() {
    let served = served();
    let ui = Ui::new(Arc::clone(&served.peer)).for_lane("s1");
    let asking = std::thread::Builder::new().name("asking".into());
    let asking = asking
        .spawn(move || ui.approve("bash", "ls", &|| false))
        .unwrap();
    let id = loop {
        if let ToClient::Ask { id, session, asked } = served.out.recv_timeout(WAIT).unwrap() {
            assert_eq!(session, "s1");
            assert!(matches!(asked, Asked::Approve { .. }));
            break id;
        }
    };
    let reply = Reply::Approval(Approval::Always);
    served
        .to_server
        .send(ToServer::Reply { id, reply })
        .unwrap();
    assert_eq!(asking.join().unwrap(), Approval::Always);
}

#[test]
fn a_question_whose_task_stopped_is_withdrawn() {
    let served = served();
    let ui = Ui::new(Arc::clone(&served.peer)).for_lane("s1");
    assert_eq!(ui.approve("bash", "ls", &|| true), Approval::No(None));
    let asked = served.out.recv_timeout(WAIT).unwrap();
    let ToClient::Ask { id, .. } = asked else {
        panic!("{asked:?}");
    };
    assert_eq!(
        served.out.recv_timeout(WAIT).unwrap(),
        ToClient::Resolved { id }
    );
}

#[test]
fn opening_listing_and_quitting_are_the_desks() {
    let served = served();
    served.call(
        1,
        Command::Open {
            target: Target::New,
        },
    );
    served.call(2, Command::List);
    served.call(3, Command::QuitAll);
    assert_eq!(served.done(3), Done::Ok);
    let asked: Vec<Request> = served.desk.try_iter().collect();
    assert_eq!(
        asked,
        [Request::Open(Target::New), Request::List, Request::Quit]
    );
}

#[test]
fn once_the_interface_is_gone_nothing_waits_for_the_person() {
    let served = served();
    let peer = Arc::clone(&served.peer);
    drop(served);
    assert_eq!(peer.inputs.next("s1"), None);
}

#[test]
fn the_place_an_interface_attaches_from_goes_to_the_desk() {
    let served = served();
    let place = protocol::Place {
        cwd: "/work/a".into(),
        env: vec![("VIRTUAL_ENV".into(), "/work/a/.venv".into())],
        theme: None,
    };
    let version = protocol::VERSION;
    let initialize = Command::Initialize {
        version,
        place: Some(place.clone()),
    };
    served.call(1, initialize);
    assert!(matches!(served.done(1), Done::Initialized { .. }));
    assert_eq!(served.desk.recv_timeout(WAIT), Ok(Request::Attach(place)));
}
